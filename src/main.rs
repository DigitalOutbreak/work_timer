use chrono::{DateTime, Local};
use csv;
use eframe::egui;
use serde::{Deserialize, Serialize};
use std::{collections::HashMap, fs, path::Path, time::Duration};
use uuid::Uuid;

fn sanitize_filename(name: &str) -> String {
    let invalid_chars = ['/', '\\', '?', '%', '*', ':', '|', '"', '<', '>', '.', ' '];
    name.chars()
        .map(|c| if invalid_chars.contains(&c) { '_' } else { c })
        .collect()
}

#[derive(Debug, Serialize, Deserialize, Clone)]
struct Task {
    id: String,
    description: String,
    folder: Option<String>,
    total_duration: i64, // Duration in seconds
    start_time: Option<DateTime<Local>>,
    is_paused: bool,
}

impl Task {
    fn new(description: String) -> Self {
        Task {
            id: Uuid::new_v4().to_string(),
            description,
            folder: None,
            total_duration: 0,
            start_time: None,
            is_paused: false,
        }
    }

    fn start(&mut self) {
        if self.start_time.is_none() && !self.is_paused {
            self.start_time = Some(Local::now());
        }
    }

    fn pause(&mut self) {
        if let Some(start) = self.start_time {
            self.total_duration += Local::now().signed_duration_since(start).num_seconds();
            self.start_time = None;
            self.is_paused = true;
        }
    }

    fn resume(&mut self) {
        if self.is_paused {
            self.start_time = Some(Local::now());
            self.is_paused = false;
        }
    }

    fn stop(&mut self) {
        if let Some(start) = self.start_time {
            self.total_duration += Local::now().signed_duration_since(start).num_seconds();
            self.start_time = None;
        }
        self.is_paused = false; // Reset paused state when completing the task
    }

    fn get_current_duration(&self) -> i64 {
        let mut duration = self.total_duration;
        if let Some(start) = self.start_time {
            duration += Local::now().signed_duration_since(start).num_seconds();
        }
        duration
    }

    fn format_duration(&self) -> String {
        let duration = self.get_current_duration();
        let hours = duration / 3600;
        let minutes = (duration % 3600) / 60;
        let seconds = duration % 60;
        format!("{:02}:{:02}:{:02}", hours, minutes, seconds)
    }
}

#[derive(Debug, Serialize, Deserialize, Clone)]
struct FolderStyle {
    name: String,
}

#[derive(Clone, Copy)]
enum TaskAction {
    Start,
    Pause,
    Resume,
    Stop,
    Delete,
}

#[derive(Default)]
struct WorkTimer {
    tasks: HashMap<String, Task>,
    folders: Vec<String>,
    folder_styles: HashMap<String, FolderStyle>,
    data_file: String,
    new_task_input: String,
    new_folder_input: String,
    selected_folder: Option<String>,
    show_new_folder_dialog: bool,
    dragged_task: Option<String>,
    show_clear_confirm: bool,
    export_message: Option<(String, f32)>, // Message and time remaining
    dark_mode: bool,
    show_shortcuts: bool,
    focus_new_task: bool,
}

impl WorkTimer {
    fn new() -> Self {
        let data_file = "tasks.json".to_string();
        let tasks = if Path::new(&data_file).exists() {
            let data = fs::read_to_string(&data_file).unwrap_or_default();
            serde_json::from_str(&data).unwrap_or_default()
        } else {
            HashMap::new()
        };

        // Load folders from file
        let folders = if Path::new("folders.json").exists() {
            let data = fs::read_to_string("folders.json").unwrap_or_default();
            serde_json::from_str(&data).unwrap_or_default()
        } else {
            Vec::new()
        };

        // Load folder styles from file
        let folder_styles = if Path::new("folder_styles.json").exists() {
            let data = fs::read_to_string("folder_styles.json").unwrap_or_default();
            serde_json::from_str(&data).unwrap_or_default()
        } else {
            HashMap::new()
        };

        let selected_folder = folders.first().cloned();

        WorkTimer {
            tasks,
            folders,
            folder_styles,
            data_file,
            new_task_input: String::new(),
            new_folder_input: String::new(),
            selected_folder,
            show_new_folder_dialog: false,
            dragged_task: None,
            show_clear_confirm: false,
            export_message: None,
            dark_mode: true,
            show_shortcuts: false,
            focus_new_task: false,
        }
    }

    fn add_task(&mut self, description: String) -> String {
        let mut task = Task::new(description);
        task.folder = self.selected_folder.clone();
        let id = task.id.clone();
        self.tasks.insert(id.clone(), task);
        self.save_tasks();
        id
    }

    fn add_folder(&mut self, name: String) {
        if !name.is_empty() && !self.folders.contains(&name) {
            let style = FolderStyle { name: name.clone() };
            self.folder_styles.insert(name.clone(), style);

            self.folders.push(name.clone());
            self.folders.sort();
            if self.selected_folder.is_none() {
                self.selected_folder = Some(name);
            }
            self.save_tasks();
            self.save_folder_styles();
        }
    }

    fn move_task_to_folder(&mut self, task_id: &str, folder: Option<String>) {
        if let Some(task) = self.tasks.get_mut(task_id) {
            task.folder = folder;
            self.save_tasks();
        }
    }

    fn save_tasks(&self) {
        if let Ok(data) = serde_json::to_string(&self.tasks) {
            let _ = fs::write(&self.data_file, data);
        }
        // Save folders to a separate file
        if let Ok(data) = serde_json::to_string(&self.folders) {
            let _ = fs::write("folders.json", data);
        }
    }

    fn get_projects(&self) -> Vec<String> {
        let mut projects: Vec<String> = self
            .tasks
            .values()
            .filter_map(|task| task.folder.clone())
            .collect::<std::collections::HashSet<_>>()
            .into_iter()
            .collect();
        if projects.is_empty() {
            projects.push("Default".to_string());
        }
        projects.sort();
        projects
    }

    fn clear_all_tasks(&mut self) {
        self.tasks.clear();
        self.save_tasks();
    }

    fn get_unique_filename(&self, base_name: &str) -> String {
        let sanitized_name = sanitize_filename(base_name);
        let mut filename = format!("{}.csv", sanitized_name);
        let mut counter = 1;

        while Path::new(&filename).exists() {
            filename = format!("{}_{}.csv", sanitized_name, counter);
            counter += 1;
        }

        filename
    }

    fn export_task_to_csv(&self, task: &Task) -> Result<String, Box<dyn std::error::Error>> {
        let filename = self.get_unique_filename(&task.description);
        let file = fs::File::create(&filename)?;
        let mut writer = csv::Writer::from_writer(file);

        // Write header
        writer.write_record(&["Task", "Duration (HH:MM:SS)", "Status"])?;

        // Write task
        let status = if task.start_time.is_some() {
            "Running"
        } else if task.is_paused {
            "Paused"
        } else {
            "Stopped"
        };

        writer.write_record(&[&task.description, &task.format_duration(), status])?;
        writer.flush()?;
        Ok(filename)
    }

    fn export_to_csv(&self) -> Result<String, Box<dyn std::error::Error>> {
        let filename = "work_timer_export.csv";
        let file = fs::File::create(filename)?;
        let mut writer = csv::Writer::from_writer(file);

        // Write header
        writer.write_record(&["Task", "Duration (HH:MM:SS)", "Status"])?;

        // Write tasks
        for task in self.tasks.values() {
            let status = if task.start_time.is_some() {
                "Running"
            } else if task.is_paused {
                "Paused"
            } else {
                "Stopped"
            };

            writer.write_record(&[&task.description, &task.format_duration(), status])?;
        }

        writer.flush()?;
        Ok(filename.to_string())
    }

    fn export_folder_to_csv(
        &self,
        folder_name: &str,
    ) -> Result<String, Box<dyn std::error::Error>> {
        let filename = format!("folder_{}.csv", sanitize_filename(folder_name));
        let file = fs::File::create(&filename)?;
        let mut writer = csv::Writer::from_writer(file);

        // Write header
        writer.write_record(&["Task", "Duration (HH:MM:SS)", "Status"])?;

        // Write tasks in this folder
        for task in self.tasks.values() {
            if task.folder.as_deref() == Some(folder_name) {
                let status = if task.start_time.is_some() {
                    "Running"
                } else if task.is_paused {
                    "Paused"
                } else {
                    "Stopped"
                };

                writer.write_record(&[&task.description, &task.format_duration(), status])?;
            }
        }

        writer.flush()?;
        Ok(filename)
    }

    fn clear_folder(&mut self, folder_name: &str) {
        self.tasks
            .retain(|_, task| task.folder.as_deref() != Some(folder_name));
        self.save_tasks();
    }

    fn save_folder_styles(&self) {
        if let Ok(data) = serde_json::to_string(&self.folder_styles) {
            let _ = fs::write("folder_styles.json", data);
        }
    }

    fn configure_theme(&self, ctx: &egui::Context) {
        let mut visuals = if self.dark_mode {
            egui::Visuals::dark()
        } else {
            egui::Visuals::light()
        };

        // Customize the theme
        if self.dark_mode {
            visuals.widgets.noninteractive.bg_fill = egui::Color32::from_rgb(30, 30, 30);
            visuals.widgets.noninteractive.fg_stroke.color = egui::Color32::from_rgb(200, 200, 200);
        } else {
            visuals.widgets.noninteractive.bg_fill = egui::Color32::from_rgb(250, 250, 250);
            visuals.widgets.noninteractive.fg_stroke.color = egui::Color32::from_rgb(50, 50, 50);
        }

        ctx.set_visuals(visuals);
    }

    fn get_folders(&self) -> Vec<String> {
        self.folders.clone()
    }

    fn get_tasks_by_folder(&self) -> HashMap<String, Vec<String>> {
        let mut tasks_by_folder: HashMap<String, Vec<String>> = HashMap::new();
        for (id, task) in self.tasks.iter() {
            let folder_name = task
                .folder
                .clone()
                .unwrap_or_else(|| "Uncategorized".to_string());
            tasks_by_folder
                .entry(folder_name)
                .or_default()
                .push(id.clone());
        }
        tasks_by_folder
    }

    fn display_task(
        &self,
        ui: &mut egui::Ui,
        task_id: &str,
        task: &Task,
    ) -> (Option<TaskAction>, Option<String>) {
        let mut action = None;
        let mut export_error = None;
        ui.horizontal(|ui| {
            ui.label(&task.description);
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                // Delete button
                if ui.button("🗑").clicked() {
                    action = Some(TaskAction::Delete);
                }

                // Export single task button
                if ui.button("📄").clicked() {
                    if let Err(e) = self.export_task_to_csv(task) {
                        export_error = Some(format!("Error exporting task: {}", e));
                    }
                }

                let button_text = if task.start_time.is_some() {
                    "⏸"
                } else if task.is_paused {
                    "▶"
                } else {
                    "▶"
                };

                if ui.button(button_text).clicked() {
                    action = Some(if task.start_time.is_some() {
                        TaskAction::Pause
                    } else if task.is_paused {
                        TaskAction::Resume
                    } else {
                        TaskAction::Start
                    });
                }

                // Show checkmark if task is running or paused
                if task.start_time.is_some() || task.is_paused {
                    if ui.button("✔").clicked() {
                        action = Some(TaskAction::Stop);
                    }
                }

                ui.label(task.format_duration());

                let status_text = if task.start_time.is_some() {
                    egui::RichText::new("Running").color(egui::Color32::GREEN)
                } else if task.is_paused {
                    egui::RichText::new("Paused").color(egui::Color32::YELLOW)
                } else if task.total_duration == 0 && !task.is_paused {
                    egui::RichText::new("Not Started").color(egui::Color32::GRAY)
                } else {
                    egui::RichText::new("Completed ✔").color(egui::Color32::from_rgb(0, 180, 180))
                };
                ui.label(status_text);
            });
        });
        (action, export_error)
    }

    fn handle_task_action(&mut self, task_id: &str, action: TaskAction) {
        match action {
            TaskAction::Delete => {
                self.tasks.remove(task_id);
                self.save_tasks();
            }
            _ => {
                if let Some(task) = self.tasks.get_mut(task_id) {
                    match action {
                        TaskAction::Start => task.start(),
                        TaskAction::Pause => task.pause(),
                        TaskAction::Resume => task.resume(),
                        TaskAction::Stop => task.stop(),
                        TaskAction::Delete => unreachable!(),
                    }
                }
            }
        }
    }
}

impl eframe::App for WorkTimer {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.configure_theme(ctx);

        // Handle keyboard shortcuts
        if ctx.input(|i| i.modifiers.command && i.key_pressed(egui::Key::N)) {
            self.show_new_folder_dialog = true;
        }
        if ctx.input(|i| i.modifiers.command && i.key_pressed(egui::Key::E)) {
            if let Err(e) = self.export_to_csv() {
                self.export_message = Some((format!("Error exporting CSV: {}", e), 3.0));
            }
        }
        if ctx.input(|i| i.modifiers.command && i.key_pressed(egui::Key::D)) {
            self.dark_mode = !self.dark_mode;
        }
        if ctx.input(|i| i.modifiers.command && i.key_pressed(egui::Key::T)) {
            self.focus_new_task = true;
        }

        egui::CentralPanel::default().show(ctx, |ui| {
            ui.heading("Work Timer");

            // Top bar with theme toggle, export and clear buttons
            ui.horizontal(|ui| {
                if ui.button(if self.dark_mode { "☀" } else { "🌙" }).clicked() {
                    self.dark_mode = !self.dark_mode;
                }

                if ui.button("⌨").clicked() {
                    self.show_shortcuts = true;
                }

                ui.separator();

                if ui.button("📊 Export All Tasks").clicked() {
                    match self.export_to_csv() {
                        Ok(filename) => {
                            self.export_message =
                                Some((format!("Tasks exported to {}", filename), 3.0));
                        }
                        Err(e) => {
                            eprintln!("Failed to export CSV: {}", e);
                            self.export_message =
                                Some((format!("Error exporting CSV: {}", e), 3.0));
                        }
                    }
                }

                if ui.button("🗑 Clear All Tasks").clicked() {
                    self.show_clear_confirm = true;
                }
            });

            // Show export message if exists
            if let Some((msg, time_left)) = &mut self.export_message {
                let color = if msg.starts_with("Error") {
                    egui::Color32::RED
                } else {
                    egui::Color32::GREEN
                };
                ui.label(egui::RichText::new(msg.clone()).color(color));
                *time_left -= ui.input(|i| i.unstable_dt);
                if *time_left <= 0.0 {
                    self.export_message = None;
                }
                ctx.request_repaint();
            }

            // Confirmation dialog for clearing all tasks
            if self.show_clear_confirm {
                egui::Window::new("Confirm Clear All")
                    .collapsible(false)
                    .resizable(false)
                    .show(ctx, |ui| {
                        ui.label(
                            "Are you sure you want to clear all tasks? This cannot be undone.",
                        );
                        ui.horizontal(|ui| {
                            if ui.button("Yes").clicked() {
                                self.clear_all_tasks();
                                self.show_clear_confirm = false;
                                self.export_message = Some(("All tasks cleared".to_string(), 3.0));
                            }
                            if ui.button("No").clicked() {
                                self.show_clear_confirm = false;
                            }
                        });
                    });
            }

            // Add the shortcuts popup window after the top bar
            if self.show_shortcuts {
                egui::Window::new("Keyboard Shortcuts")
                    .collapsible(false)
                    .resizable(false)
                    .show(ctx, |ui| {
                        ui.label("Global Shortcuts:");
                        ui.add_space(4.0);

                        egui::Grid::new("shortcuts_grid")
                            .num_columns(2)
                            .spacing([40.0, 4.0])
                            .show(ui, |ui| {
                                ui.label("⌘T");
                                ui.label("New Task");
                                ui.end_row();

                                ui.label("⌘D");
                                ui.label("Toggle Dark/Light Mode");
                                ui.end_row();

                                ui.label("⌘E");
                                ui.label("Export All Tasks");
                                ui.end_row();

                                ui.label("⌘N");
                                ui.label("New Folder");
                                ui.end_row();

                                ui.label("Enter");
                                ui.label("Create Task/Folder");
                                ui.end_row();
                            });

                        ui.add_space(8.0);
                        ui.horizontal(|ui| {
                            if ui.button("Close").clicked() {
                                self.show_shortcuts = false;
                            }
                        });
                    });
            }

            ui.add_space(16.0);

            // Folder selection and creation
            ui.horizontal(|ui| {
                ui.label("Folder:");
                if ui.button("📁 New Folder").clicked() {
                    self.show_new_folder_dialog = true;
                }
            });

            // New folder dialog
            if self.show_new_folder_dialog {
                egui::Window::new("New Folder")
                    .collapsible(false)
                    .resizable(false)
                    .show(ctx, |ui| {
                        ui.horizontal(|ui| {
                            let text_edit = ui.text_edit_singleline(&mut self.new_folder_input);
                            if ui.button("Create").clicked()
                                || (text_edit.lost_focus()
                                    && ui.input(|i| i.key_pressed(egui::Key::Enter)))
                            {
                                if !self.new_folder_input.trim().is_empty() {
                                    self.add_folder(self.new_folder_input.trim().to_string());
                                    self.new_folder_input.clear();
                                    self.show_new_folder_dialog = false;
                                }
                            }
                            if ui.button("Cancel").clicked() {
                                self.show_new_folder_dialog = false;
                                self.new_folder_input.clear();
                            }
                        });
                    });
            }

            ui.add_space(16.0);

            // New task input with folder selection
            ui.horizontal(|ui| {
                ui.label("Add task to folder:");
                let selected_text = self
                    .selected_folder
                    .as_ref()
                    .map(String::as_str)
                    .unwrap_or("No folders");
                egui::ComboBox::from_id_source("folder_selector")
                    .selected_text(selected_text)
                    .show_ui(ui, |ui| {
                        for folder in &self.folders {
                            let folder_str = folder.as_str();
                            ui.selectable_value(
                                &mut self.selected_folder,
                                Some(folder_str.to_string()),
                                folder_str,
                            );
                        }
                    });
            });

            ui.horizontal(|ui| {
                let response = ui.text_edit_singleline(&mut self.new_task_input);
                if self.focus_new_task {
                    response.request_focus();
                    self.focus_new_task = false;
                }
                if ui.button("Add Task").clicked()
                    || (response.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)))
                {
                    if !self.new_task_input.trim().is_empty() {
                        self.add_task(self.new_task_input.trim().to_string());
                        self.new_task_input.clear();
                    }
                }
            });

            ui.add_space(16.0);

            // Display tasks by folder with custom colors
            egui::ScrollArea::vertical().show(ui, |ui| {
                let folders = self.get_folders();
                let tasks_by_folder = self.get_tasks_by_folder();

                for folder in folders {
                    if let Some(task_ids) = tasks_by_folder.get(&folder) {
                        let folder_name = folder.clone();

                        egui::Frame::none()
                            .outer_margin(egui::style::Margin::symmetric(0.0, 2.0))
                            .show(ui, |ui| {
                                ui.collapsing(folder_name.clone(), |ui| {
                                    // Add folder actions in a more compact layout
                                    ui.horizontal(|ui| {
                                        if ui.button("📊").clicked() {
                                            match self.export_folder_to_csv(&folder_name) {
                                                Ok(filename) => {
                                                    self.export_message = Some((
                                                        format!("Folder exported to {}", filename),
                                                        3.0,
                                                    ));
                                                }
                                                Err(e) => {
                                                    self.export_message = Some((
                                                        format!("Error exporting folder: {}", e),
                                                        3.0,
                                                    ));
                                                }
                                            }
                                        }
                                        ui.small("Export");

                                        if ui.button("🗑").clicked() {
                                            self.clear_folder(&folder_name);
                                        }
                                        ui.small("Clear");
                                    });

                                    // Display tasks in the folder
                                    for task_id in task_ids {
                                        if let Some(task) = self.tasks.get(task_id) {
                                            let (action, export_error) =
                                                self.display_task(ui, task_id, task);
                                            if let Some(action) = action {
                                                self.handle_task_action(task_id, action);
                                            }
                                            if let Some(error) = export_error {
                                                self.export_message = Some((error, 3.0));
                                            }
                                        }
                                    }
                                });
                            });
                    }
                }
            });
        });

        // Request repaint for timer updates
        if self.tasks.values().any(|task| task.start_time.is_some()) {
            ctx.request_repaint();
        }
    }
}

fn main() -> Result<(), eframe::Error> {
    let options = eframe::NativeOptions {
        window_builder: Some(Box::new(|builder| {
            builder.with_inner_size(egui::Vec2::new(480.0, 640.0))
        })),
        ..Default::default()
    };

    eframe::run_native(
        "Work Timer",
        options,
        Box::new(|_cc| Box::new(WorkTimer::new())),
    )
}
