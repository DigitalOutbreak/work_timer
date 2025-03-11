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

#[derive(Default)]
struct WorkTimer {
    tasks: HashMap<String, Task>,
    folders: Vec<String>,
    data_file: String,
    new_task_input: String,
    new_folder_input: String,
    selected_folder: Option<String>,
    show_new_folder_dialog: bool,
    dragged_task: Option<String>,
    show_clear_confirm: bool,
    export_message: Option<(String, f32)>, // Message and time remaining
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

        let selected_folder = folders.first().cloned();

        WorkTimer {
            tasks,
            folders,
            data_file,
            new_task_input: String::new(),
            new_folder_input: String::new(),
            selected_folder,
            show_new_folder_dialog: false,
            dragged_task: None,
            show_clear_confirm: false,
            export_message: None,
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
            self.folders.push(name.clone());
            self.folders.sort();
            if self.selected_folder.is_none() {
                self.selected_folder = Some(name);
            }
            self.save_tasks();
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
}

impl eframe::App for WorkTimer {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        egui::CentralPanel::default().show(ctx, |ui| {
            ui.heading("Work Timer");

            // Top bar with export and clear buttons
            ui.horizontal(|ui| {
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
            });

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
                egui::ComboBox::from_id_source("folder_selector")
                    .selected_text(self.selected_folder.as_deref().unwrap_or("No folders"))
                    .show_ui(ui, |ui| {
                        for folder in &self.folders {
                            ui.selectable_value(
                                &mut self.selected_folder,
                                Some(folder.clone()),
                                folder,
                            );
                        }
                    });
            });

            ui.horizontal(|ui| {
                let text_edit = ui.text_edit_singleline(&mut self.new_task_input);
                if ui.button("Add Task").clicked()
                    || (text_edit.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)))
                {
                    if !self.new_task_input.trim().is_empty() {
                        let id = self.add_task(self.new_task_input.trim().to_string());
                        if let Some(task) = self.tasks.get_mut(&id) {
                            task.start();
                        }
                        self.new_task_input.clear();
                    }
                }
            });

            ui.add_space(16.0);

            // Tasks list grouped by folders
            egui::ScrollArea::vertical().show(ui, |ui| {
                // First collect all tasks by folder using only IDs
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

                let mut tasks_to_remove = Vec::new();
                let mut task_actions = Vec::new();
                let mut task_to_export = None;

                // Sort folders for consistent display order
                let mut folders: Vec<String> = tasks_by_folder.keys().cloned().collect();
                folders.sort();

                // Display tasks by folder
                for folder in folders {
                    if let Some(task_ids) = tasks_by_folder.get(&folder) {
                        ui.collapsing(folder.clone(), |ui| {
                            // Add folder actions
                            ui.horizontal(|ui| {
                                if ui.button("📊 Export Folder").clicked() {
                                    match self.export_folder_to_csv(&folder) {
                                        Ok(filename) => {
                                            self.export_message = Some((
                                                format!("Folder exported to {}", filename),
                                                3.0,
                                            ));
                                        }
                                        Err(e) => {
                                            eprintln!("Failed to export folder CSV: {}", e);
                                            self.export_message = Some((
                                                format!("Error exporting folder: {}", e),
                                                3.0,
                                            ));
                                        }
                                    }
                                }
                                if ui.button("🗑 Clear Folder").clicked() {
                                    self.clear_folder(&folder);
                                    self.export_message =
                                        Some((format!("Cleared folder {}", folder), 3.0));
                                }
                            });
                            ui.add_space(8.0);

                            // Sort task IDs for consistent display order
                            let mut sorted_ids = task_ids.clone();
                            // Create a string to own during sorting
                            let empty_string = String::new();
                            sorted_ids.sort_by_key(|id| {
                                self.tasks
                                    .get(id)
                                    .map(|t| t.description.clone())
                                    .unwrap_or_else(|| empty_string.clone())
                            });

                            for id in sorted_ids {
                                if let Some(task) = self.tasks.get(&id) {
                                    ui.horizontal(|ui| {
                                        ui.label(&task.description);
                                        ui.with_layout(
                                            egui::Layout::right_to_left(egui::Align::Center),
                                            |ui| {
                                                // Export single task button
                                                if ui.button("📄").clicked() {
                                                    task_to_export = Some(id.clone());
                                                }
                                                ui.small("Export");

                                                if ui.button("🗑").clicked() {
                                                    tasks_to_remove.push(id.clone());
                                                }

                                                let button_text = if task.start_time.is_some() {
                                                    "⏸"
                                                } else if task.is_paused {
                                                    "▶"
                                                } else {
                                                    "▶"
                                                };

                                                if ui.button(button_text).clicked() {
                                                    let id = id.clone();
                                                    if task.start_time.is_some() {
                                                        task_actions.push((id, "pause"));
                                                    } else if task.is_paused {
                                                        task_actions.push((id, "resume"));
                                                    } else {
                                                        task_actions.push((id, "start"));
                                                    }
                                                }

                                                if task.start_time.is_some() || task.is_paused {
                                                    if ui.button("⏹").clicked() {
                                                        task_actions.push((id.clone(), "stop"));
                                                    }
                                                }

                                                ui.label(task.format_duration());

                                                let status_text = if task.start_time.is_some() {
                                                    egui::RichText::new("Running")
                                                        .color(egui::Color32::GREEN)
                                                } else if task.is_paused {
                                                    egui::RichText::new("Paused")
                                                        .color(egui::Color32::YELLOW)
                                                } else {
                                                    egui::RichText::new("Stopped")
                                                        .color(egui::Color32::RED)
                                                };
                                                ui.label(status_text);
                                            },
                                        );
                                    });
                                    ui.add_space(4.0);
                                }
                            }
                        });
                    }
                }

                // Handle task export
                if let Some(id) = task_to_export {
                    if let Some(task) = self.tasks.get(&id) {
                        match self.export_task_to_csv(task) {
                            Ok(filename) => {
                                self.export_message =
                                    Some((format!("Task exported to {}", filename), 3.0));
                            }
                            Err(e) => {
                                eprintln!("Failed to export task CSV: {}", e);
                                self.export_message =
                                    Some((format!("Error exporting task: {}", e), 3.0));
                            }
                        }
                    }
                }

                // Apply task actions
                for (id, action) in task_actions {
                    if let Some(task) = self.tasks.get_mut(&id) {
                        match action {
                            "pause" => task.pause(),
                            "resume" => task.resume(),
                            "start" => task.start(),
                            "stop" => task.stop(),
                            _ => {}
                        }
                    }
                }

                // Remove tasks marked for deletion
                for id in &tasks_to_remove {
                    self.tasks.remove(id);
                }
                if !tasks_to_remove.is_empty() {
                    self.save_tasks();
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
