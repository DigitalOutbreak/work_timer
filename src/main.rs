use chrono::{DateTime, Local};
use csv;
use eframe::egui;
use egui_phosphor::fill;
use serde::{Deserialize, Serialize};
use std::{collections::HashMap, fs, path::Path};
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
    Delete,
    Complete,
}

#[derive(Clone, Copy, PartialEq)]
enum StatsTab {
    Overview,
    Projects,
    Timeline,
    Details,
}

impl Default for StatsTab {
    fn default() -> Self {
        StatsTab::Overview
    }
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
    show_clear_folders_confirm: bool,
    dragged_task: Option<String>,
    show_clear_confirm: bool,
    show_clear_folder_confirm: Option<String>,
    show_delete_task_confirm: Option<String>,
    export_message: Option<(String, f32)>,
    dark_mode: bool,
    show_shortcuts: bool,
    show_settings: bool,
    show_statistics: bool,
    selected_stats_tab: StatsTab,
    ui_scale: f32,
    temporary_ui_scale: f32,
    focus_new_task: bool,
    focus_new_folder: bool,
    show_add_task_dialog: bool,
    add_task_to_folder: Option<String>,
    new_task_in_folder: String,
    dragged_folder: Option<String>,
    focused_folder_index: Option<usize>,
    focused_task_index: Option<usize>,
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
        let default_scale = 2.0;
        let focused_folder_index = if !folders.is_empty() { Some(0) } else { None };
        let focused_task_index = None;

        WorkTimer {
            tasks,
            folders,
            folder_styles,
            data_file,
            new_task_input: String::new(),
            new_folder_input: String::new(),
            selected_folder,
            show_new_folder_dialog: false,
            show_clear_folders_confirm: false,
            dragged_task: None,
            show_clear_confirm: false,
            show_clear_folder_confirm: None,
            show_delete_task_confirm: None,
            export_message: None,
            dark_mode: true,
            show_shortcuts: false,
            show_settings: false,
            show_statistics: false,
            selected_stats_tab: StatsTab::Overview,
            ui_scale: default_scale,
            temporary_ui_scale: default_scale,
            focus_new_task: false,
            focus_new_folder: false,
            show_add_task_dialog: false,
            add_task_to_folder: None,
            new_task_in_folder: String::new(),
            dragged_folder: None,
            focused_folder_index,
            focused_task_index,
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
                self.selected_folder = Some(name.clone());
            }
            // Find the index of the newly added folder after sorting
            if let Some(new_folder_idx) = self.folders.iter().position(|f| f == &name) {
                self.focused_folder_index = Some(new_folder_idx);
                self.focused_task_index = None; // Reset task focus when switching folders
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
        
        // Clean up CSV files
        let _ = fs::remove_file("work_timer_export.csv"); // Remove main export file
        
        // Remove individual task exports
        if let Ok(entries) = fs::read_dir(".") {
            for entry in entries.flatten() {
                if let Ok(file_name) = entry.file_name().into_string() {
                    if file_name.ends_with(".csv") {
                        let _ = fs::remove_file(&file_name);
                    }
                }
            }
        }
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
        writer.write_record(&["Task", "Project", "Duration (HH:MM:SS)", "Status"])?;

        // Write task
        let status = if task.start_time.is_some() {
            "Running"
        } else if task.is_paused {
            "Paused"
        } else {
            "Stopped"
        };

        writer.write_record(&[
            &task.description,
            task.folder.as_deref().unwrap_or("Uncategorized"),
            &task.format_duration(),
            status
        ])?;
        writer.flush()?;
        Ok(filename)
    }

    fn export_to_csv(&self) -> Result<String, Box<dyn std::error::Error>> {
        let filename = "work_timer_export.csv";
        let file = fs::File::create(filename)?;
        let mut writer = csv::Writer::from_writer(file);

        // Write header
        writer.write_record(&["Task", "Project", "Duration (HH:MM:SS)", "Status"])?;

        // Write tasks
        for task in self.tasks.values() {
            let status = if task.start_time.is_some() {
                "Running"
            } else if task.is_paused {
                "Paused"
            } else {
                "Stopped"
            };

            writer.write_record(&[
                &task.description,
                task.folder.as_deref().unwrap_or("Uncategorized"),
                &task.format_duration(),
                status
            ])?;
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
        writer.write_record(&["Task", "Project", "Duration (HH:MM:SS)", "Status"])?;

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

                writer.write_record(&[
                    &task.description,
                    folder_name,
                    &task.format_duration(),
                    status
                ])?;
            }
        }

        writer.flush()?;
        Ok(filename)
    }

    fn clear_folder(&mut self, folder_name: &str) {
        // Remove the folder's CSV export if it exists
        let folder_csv = format!("folder_{}.csv", sanitize_filename(folder_name));
        let _ = fs::remove_file(&folder_csv);

        // Remove individual task CSV files for tasks in this folder and the tasks themselves
        self.tasks.retain(|_, task| {
            if task.folder.as_deref() == Some(folder_name) {
                // Remove the task's CSV file if it exists
                let _ = fs::remove_file(format!("{}.csv", sanitize_filename(&task.description)));
                false // Remove this task
            } else {
                true // Keep tasks from other folders
            }
        });

        // Remove the folder from the folders list
        if let Some(index) = self.folders.iter().position(|f| f == folder_name) {
            self.folders.remove(index);
            self.folder_styles.remove(folder_name);
            // If this was the selected folder, clear the selection
            if self.selected_folder.as_deref() == Some(folder_name) {
                self.selected_folder = self.folders.first().cloned();
            }
            // Update focused folder index if needed
            if let Some(focused_idx) = self.focused_folder_index {
                if focused_idx >= self.folders.len() {
                    self.focused_folder_index = if self.folders.is_empty() {
                        None
                    } else {
                        Some(self.folders.len() - 1)
                    };
                }
            }
            self.save_tasks();
            self.save_folder_styles();
        }
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
        
        // Customize colors based on theme
        if self.dark_mode {
            visuals.override_text_color = Some(egui::Color32::from_rgb(230, 230, 230));
            visuals.widgets.noninteractive.bg_fill = egui::Color32::from_rgb(32, 33, 36);
            visuals.widgets.inactive.bg_fill = egui::Color32::from_rgb(45, 45, 48);
            visuals.widgets.hovered.bg_fill = egui::Color32::from_rgb(55, 55, 58);
            visuals.widgets.active.bg_fill = egui::Color32::from_rgb(48, 48, 51);
            visuals.window_fill = egui::Color32::from_rgb(32, 33, 36);
            visuals.panel_fill = egui::Color32::from_rgb(32, 33, 36);
        } else {
            visuals.override_text_color = Some(egui::Color32::from_rgb(25, 25, 25));
            visuals.widgets.noninteractive.bg_fill = egui::Color32::from_rgb(252, 252, 252);
            visuals.widgets.inactive.bg_fill = egui::Color32::from_rgb(248, 248, 248);
            visuals.widgets.hovered.bg_fill = egui::Color32::from_rgb(240, 240, 240);
            visuals.widgets.active.bg_fill = egui::Color32::from_rgb(235, 235, 235);
            visuals.window_fill = egui::Color32::from_rgb(252, 252, 252);
            visuals.panel_fill = egui::Color32::from_rgb(252, 252, 252);
        }
        
        // Apply the styles
        ctx.set_visuals(visuals);
        ctx.set_pixels_per_point(self.ui_scale);
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
            // Complete button (checkbox style) on the left
            let is_completed = task.total_duration > 0 && task.start_time.is_none() && !task.is_paused;
            let complete_icon = if is_completed {
                fill::CHECK_SQUARE
            } else {
                fill::SQUARE
            };
            if ui.button(complete_icon).clicked() {
                action = Some(TaskAction::Complete);
            }
            
            ui.label(&task.description);
            
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                // Delete button
                if ui.button(fill::TRASH).clicked() {
                    action = Some(TaskAction::Delete);
                }

                // Export single task button
                if ui.button(fill::EXPORT).clicked() {
                    if let Err(e) = self.export_task_to_csv(task) {
                        export_error = Some(format!("Error exporting task: {}", e));
                    }
                }

                // Only show play/pause button if task is not completed
                if !is_completed {
                    let button_text = if task.start_time.is_some() {
                        fill::PAUSE // Pause icon
                    } else if task.is_paused {
                        fill::PLAY // Play icon
                    } else {
                        fill::PLAY // Play icon
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
                }

                ui.label(task.format_duration());

                let status_text = if task.start_time.is_some() {
                    egui::RichText::new("Running").color(egui::Color32::GREEN)
                } else if task.is_paused {
                    egui::RichText::new("Paused").color(egui::Color32::YELLOW)
                } else if task.total_duration == 0 && !task.is_paused {
                    egui::RichText::new("Not Started").color(egui::Color32::GRAY)
                } else {
                    egui::RichText::new("Completed").color(egui::Color32::from_rgb(0, 180, 180))
                };
                ui.label(status_text);
            });
        });
        (action, export_error)
    }

    fn handle_task_action(&mut self, task_id: &str, action: TaskAction) {
        match action {
            TaskAction::Delete => {
                self.show_delete_task_confirm = Some(task_id.to_string());
            }
            TaskAction::Complete => {
                if let Some(task) = self.tasks.get_mut(task_id) {
                    let is_completed = task.total_duration > 0 && task.start_time.is_none() && !task.is_paused;
                    if is_completed {
                        // If task is completed, mark it as incomplete by setting is_paused to true
                        task.is_paused = true;
                    } else {
                        // If task is not completed, mark it as completed
                        if task.start_time.is_some() {
                            task.pause(); // Stop the timer if it's running
                        }
                        task.is_paused = false; // Mark as not paused
                    }
                    self.save_tasks();
                }
            }
            _ => {
                if let Some(task) = self.tasks.get_mut(task_id) {
                    match action {
                        TaskAction::Start => task.start(),
                        TaskAction::Pause => task.pause(),
                        TaskAction::Resume => task.resume(),
                        TaskAction::Delete | TaskAction::Complete => unreachable!(),
                    }
                }
            }
        }
    }

    fn clear_all_folders(&mut self) {
        self.folders.clear();
        self.folder_styles.clear();
        self.selected_folder = None;
        // Reset focus but don't set to None - it will be set to Some(0) when a new folder is added
        self.focused_folder_index = None;
        self.focused_task_index = None;
        self.save_tasks();
        self.save_folder_styles();
    }

    fn calculate_folder_durations(&self) -> Vec<(String, i64)> {
        let mut durations: HashMap<String, i64> = HashMap::new();
        
        for task in self.tasks.values() {
            let folder = task.folder.clone().unwrap_or_else(|| "Uncategorized".to_string());
            *durations.entry(folder).or_default() += task.get_current_duration();
        }

        let mut result: Vec<_> = durations.into_iter().collect();
        result.sort_by_key(|(_, duration)| std::cmp::Reverse(*duration));
        result
    }

    fn calculate_average_task_duration(&self) -> i64 {
        if self.tasks.is_empty() {
            return 0;
        }
        let total: i64 = self.tasks.values().map(|t| t.get_current_duration()).sum();
        total / self.tasks.len() as i64
    }

    fn format_duration(seconds: i64) -> String {
        let hours = seconds / 3600;
        let minutes = (seconds % 3600) / 60;
        let seconds = seconds % 60;
        format!("{:02}:{:02}:{:02}", hours, minutes, seconds)
    }

    fn is_any_dialog_open(&self) -> bool {
        self.show_new_folder_dialog || 
        self.show_clear_folders_confirm || 
        self.show_clear_confirm || 
        self.show_clear_folder_confirm.is_some() || 
        self.show_delete_task_confirm.is_some() || 
        self.show_shortcuts || 
        self.show_settings || 
        self.show_add_task_dialog ||
        self.show_statistics
    }
}

impl eframe::App for WorkTimer {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.configure_theme(ctx);

        // Handle global shortcuts that should work even when dialogs are open
        if ctx.input(|i| i.modifiers.command && i.key_pressed(egui::Key::D)) {
            self.dark_mode = !self.dark_mode;
        }

        // Handle dialog closing with Escape or Cmd+W
        if ctx.input(|i| i.key_pressed(egui::Key::Escape) || (i.modifiers.command && i.key_pressed(egui::Key::W))) {
            if self.show_new_folder_dialog {
                self.show_new_folder_dialog = false;
                self.new_folder_input.clear();
            } else if self.show_clear_folders_confirm {
                self.show_clear_folders_confirm = false;
            } else if self.show_clear_confirm {
                self.show_clear_confirm = false;
            } else if self.show_clear_folder_confirm.is_some() {
                self.show_clear_folder_confirm = None;
            } else if self.show_delete_task_confirm.is_some() {
                self.show_delete_task_confirm = None;
            } else if self.show_shortcuts {
                self.show_shortcuts = false;
            } else if self.show_settings {
                self.temporary_ui_scale = self.ui_scale; // Reset temporary scale
                self.show_settings = false;
            } else if self.show_add_task_dialog {
                self.show_add_task_dialog = false;
                self.add_task_to_folder = None;
                self.new_task_in_folder.clear();
            } else if self.show_statistics {
                self.show_statistics = false;
            }
        }

        // Handle keyboard shortcuts and navigation
        if !self.is_any_dialog_open() {
            // Handle space bar for play/pause
            if ctx.input(|i| i.key_pressed(egui::Key::Space)) {
                let folders = self.get_folders();
                if let Some(current_folder_idx) = self.focused_folder_index {
                    let folder_name = &folders[current_folder_idx];
                    let folder_id = egui::Id::new(format!("folder_{}", folder_name));
                    let is_open = ctx.memory(|mem| mem.data.get_temp::<bool>(folder_id).unwrap_or(true));
                    
                    // Only handle space if we have a focused task in an open folder
                    if is_open && self.focused_task_index.is_some() {
                        let tasks = self.get_tasks_by_folder();
                        if let Some(task_ids) = tasks.get(folder_name.as_str()) {
                            if let Some(task_idx) = self.focused_task_index {
                                if let Some(task) = self.tasks.get(task_ids[task_idx].as_str()) {
                                    let action = if task.start_time.is_some() {
                                        TaskAction::Pause
                                    } else if task.is_paused {
                                        TaskAction::Resume
                                    } else {
                                        TaskAction::Start
                                    };
                                    self.handle_task_action(task_ids[task_idx].as_str(), action);
                                }
                            }
                        }
                    }
                }
            }

            // Handle Cmd+Delete for focused item
            if ctx.input(|i| i.modifiers.command && (i.key_pressed(egui::Key::Backspace) || i.key_pressed(egui::Key::Delete))) {
                let folders = self.get_folders();
                if let Some(current_folder_idx) = self.focused_folder_index {
                    let folder_name = &folders[current_folder_idx];
                    let folder_id = egui::Id::new(format!("folder_{}", folder_name));
                    let is_open = ctx.memory(|mem| mem.data.get_temp::<bool>(folder_id).unwrap_or(true));
                    
                    // If we have a focused task in an open folder, delete the task
                    if is_open && self.focused_task_index.is_some() {
                        let tasks = self.get_tasks_by_folder();
                        if let Some(task_ids) = tasks.get(folder_name.as_str()) {
                            if let Some(task_idx) = self.focused_task_index {
                                self.show_delete_task_confirm = Some(task_ids[task_idx].clone());
                            }
                        }
                    } else {
                        // If we're on a folder header, delete the folder
                        self.show_clear_folder_confirm = Some(folder_name.clone());
                    }
                }
            }

            if ctx.input(|i| i.key_pressed(egui::Key::ArrowUp)) {
                let folders = self.get_folders();
                if let Some(current_folder_idx) = self.focused_folder_index {
                    let folder_name = &folders[current_folder_idx];
                    let folder_id = egui::Id::new(format!("folder_{}", folder_name));
                    let is_open = ctx.memory(|mem| mem.data.get_temp::<bool>(folder_id).unwrap_or(true));
                    
                    if is_open && self.focused_task_index.is_some() {
                        // If we're focused on a task, move up through tasks
                        if let Some(current_task_idx) = self.focused_task_index {
                            if current_task_idx > 0 {
                                self.focused_task_index = Some(current_task_idx - 1);
                            } else {
                                // If at first task, move to folder header
                                self.focused_task_index = None;
                            }
                        }
                    } else {
                        // If we're on a folder header, move to previous folder
                        if current_folder_idx > 0 {
                            self.focused_folder_index = Some(current_folder_idx - 1);
                            self.focused_task_index = None;
                        }
                    }
                }
            }

            if ctx.input(|i| i.key_pressed(egui::Key::ArrowDown)) {
                let folders = self.get_folders();
                if let Some(current_folder_idx) = self.focused_folder_index {
                    let folder_name = &folders[current_folder_idx];
                    let folder_id = egui::Id::new(format!("folder_{}", folder_name));
                    let is_open = ctx.memory(|mem| mem.data.get_temp::<bool>(folder_id).unwrap_or(true));
                    let tasks = self.get_tasks_by_folder();
                    let task_ids = tasks.get(folder_name.as_str()).cloned().unwrap_or_default();
                    
                    if is_open && !task_ids.is_empty() {
                        // If folder is open and has tasks
                        if self.focused_task_index.is_none() {
                            // If on folder header, move to first task
                            self.focused_task_index = Some(0);
                        } else if let Some(current_task_idx) = self.focused_task_index {
                            // If on a task, try to move to next task
                            if current_task_idx < task_ids.len() - 1 {
                                self.focused_task_index = Some(current_task_idx + 1);
                            } else {
                                // If at last task, move to next folder
                                if current_folder_idx < folders.len() - 1 {
                                    self.focused_folder_index = Some(current_folder_idx + 1);
                                    self.focused_task_index = None;
                                }
                            }
                        }
                    } else {
                        // If folder is closed or empty, move to next folder
                        if current_folder_idx < folders.len() - 1 {
                            self.focused_folder_index = Some(current_folder_idx + 1);
                            self.focused_task_index = None;
                        }
                    }
                }
            }
        }

        // Handle keyboard shortcuts only when no dialog is open
        if !self.is_any_dialog_open() {
            if ctx.input(|i| i.modifiers.command && i.key_pressed(egui::Key::N)) {
                self.show_new_folder_dialog = true;
                self.focus_new_folder = true;
            }
            if ctx.input(|i| i.modifiers.command && i.key_pressed(egui::Key::E)) {
                if let Err(e) = self.export_to_csv() {
                    self.export_message = Some((format!("Error exporting CSV: {}", e), 3.0));
                }
            }
            if ctx.input(|i| i.modifiers.command && i.key_pressed(egui::Key::T)) {
                if let Some(focused_idx) = self.focused_folder_index {
                    // If a folder is focused, open the add task dialog for that folder
                    if let Some(folder_name) = self.folders.get(focused_idx) {
                        self.show_add_task_dialog = true;
                        self.add_task_to_folder = Some(folder_name.clone());
                        self.new_task_in_folder.clear();
                    }
                } else {
                    // If no folder is focused, focus the quick add task input
                    self.focus_new_task = true;
                }
            }
            if ctx.input(|i| i.modifiers.command && i.key_pressed(egui::Key::S)) {
                self.show_statistics = true;
            }
            if ctx.input(|i| i.modifiers.command && i.key_pressed(egui::Key::Comma)) {
                self.show_settings = true;
            }
        }

        egui::CentralPanel::default().show(ctx, |ui| {
            ui.heading("Work Timer");

            // Top bar with theme toggle, export and clear buttons
            ui.horizontal(|ui| {
                if ui.button(if self.dark_mode { "☀" } else { "🌙" }).clicked() {
                    self.dark_mode = !self.dark_mode;
                }

                if ui.button("⚙").clicked() {
                    self.show_settings = true;
                }

                if ui.button("⌨").clicked() {
                    self.show_shortcuts = true;
                }

                if ui.button("📊").clicked() {
                    self.show_statistics = true;
                }

                ui.separator();

                if !self.tasks.is_empty() {
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
                            ui.spacing_mut().item_spacing.x = 10.0;
                            let yes_button = ui.add(egui::Button::new("Yes"));
                            let no_button = ui.add(egui::Button::new("No"));
                            
                            let dialog_id = ui.id().with("clear_all_dialog");
                            let focus_id = dialog_id.with("focus");
                            
                            // Initialize focus to "yes" if not set
                            if !ui.memory(|mem| mem.data.get_temp::<bool>(focus_id).is_some()) {
                                ui.memory_mut(|mem| mem.data.insert_temp(focus_id, true));  // true = yes focused
                            }

                            let mut yes_focused = ui.memory(|mem| mem.data.get_temp::<bool>(focus_id).unwrap_or(true));

                            // Handle tab navigation
                            if ui.input(|i| i.key_pressed(egui::Key::Tab)) {
                                yes_focused = !yes_focused;
                                ui.memory_mut(|mem| mem.data.insert_temp(focus_id, yes_focused));
                            }

                            // Apply focus based on memory state
                            if yes_focused {
                                yes_button.request_focus();
                            } else {
                                no_button.request_focus();
                            }

                            if yes_button.clicked() || (yes_button.has_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter))) {
                                self.clear_all_tasks();
                                self.show_clear_confirm = false;
                                self.export_message = Some(("All tasks cleared".to_string(), 3.0));
                            }
                            if no_button.clicked() || (no_button.has_focus() && (ui.input(|i| i.key_pressed(egui::Key::Enter)) || ui.input(|i| i.key_pressed(egui::Key::Escape)))) {
                                self.show_clear_confirm = false;
                            }
                        });
                    });
            }

            // Confirmation dialog for clearing a folder
            if let Some(folder_name) = &self.show_clear_folder_confirm.clone() {
                let folder_name = folder_name.clone();
                egui::Window::new(format!("Clear Folder '{}'", folder_name))
                    .collapsible(false)
                    .resizable(false)
                    .show(ctx, |ui| {
                        ui.label(format!(
                            "Are you sure you want to delete the folder '{}'? This will remove the folder and all its tasks. This cannot be undone.",
                            folder_name
                        ));
                        ui.horizontal(|ui| {
                            ui.spacing_mut().item_spacing.x = 10.0;
                            let yes_button = ui.add(egui::Button::new("Yes"));
                            let no_button = ui.add(egui::Button::new("No"));
                            
                            let dialog_id = ui.id().with("clear_folder_dialog");
                            let focus_id = dialog_id.with("focus");
                            
                            // Initialize focus to "yes" only if focus state doesn't exist yet
                            if !ui.memory(|mem| mem.data.get_temp::<bool>(focus_id).is_some()) {
                                ui.memory_mut(|mem| mem.data.insert_temp(focus_id, true));
                            }

                            let mut yes_focused = ui.memory(|mem| mem.data.get_temp::<bool>(focus_id).unwrap_or(true));

                            // Handle tab navigation
                            if ui.input(|i| i.key_pressed(egui::Key::Tab)) {
                                yes_focused = !yes_focused;
                                ui.memory_mut(|mem| mem.data.insert_temp(focus_id, yes_focused));
                            }

                            // Apply focus based on memory state
                            if yes_focused {
                                yes_button.request_focus();
                            } else {
                                no_button.request_focus();
                            }

                            if yes_button.clicked() || (yes_button.has_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter))) {
                                self.clear_folder(&folder_name);
                                self.show_clear_folder_confirm = None;
                                // Clear the focus state from memory when closing
                                ui.memory_mut(|mem| mem.data.remove::<bool>(focus_id));
                                self.export_message = Some((format!("Folder '{}' deleted", folder_name), 3.0));
                            }
                            if no_button.clicked() || (no_button.has_focus() && (ui.input(|i| i.key_pressed(egui::Key::Enter)) || ui.input(|i| i.key_pressed(egui::Key::Escape)))) {
                                self.show_clear_folder_confirm = None;
                                // Clear the focus state from memory when closing
                                ui.memory_mut(|mem| mem.data.remove::<bool>(focus_id));
                            }
                        });
                    });
            }

            // Confirmation dialog for deleting a task
            if let Some(task_id) = &self.show_delete_task_confirm.clone() {
                let task_id = task_id.clone();
                let task_info = self.tasks.get(&task_id).map(|task| (task.description.clone()));
                if let Some(task_description) = task_info {
                    egui::Window::new("Delete Task")
                        .collapsible(false)
                        .resizable(false)
                        .show(ctx, |ui| {
                            ui.label(format!(
                                "Are you sure you want to delete task '{}'? This cannot be undone.",
                                task_description
                            ));
                            ui.horizontal(|ui| {
                                ui.spacing_mut().item_spacing.x = 10.0;
                                let yes_button = ui.add(egui::Button::new("Yes"));
                                let no_button = ui.add(egui::Button::new("No"));
                                
                                let dialog_id = ui.id().with("delete_task_dialog");
                                let focus_id = dialog_id.with("focus");
                                
                                // Initialize focus to "yes" if not set
                                if !ui.memory(|mem| mem.data.get_temp::<bool>(focus_id).is_some()) {
                                    ui.memory_mut(|mem| mem.data.insert_temp(focus_id, true));  // true = yes focused
                                }

                                let mut yes_focused = ui.memory(|mem| mem.data.get_temp::<bool>(focus_id).unwrap_or(true));

                                // Handle tab navigation
                                if ui.input(|i| i.key_pressed(egui::Key::Tab)) {
                                    yes_focused = !yes_focused;
                                    ui.memory_mut(|mem| mem.data.insert_temp(focus_id, yes_focused));
                                }

                                // Apply focus based on memory state
                                if yes_focused {
                                    yes_button.request_focus();
                                } else {
                                    no_button.request_focus();
                                }

                                if yes_button.clicked() || (yes_button.has_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter))) {
                                    self.tasks.remove(&task_id);
                                    self.save_tasks();
                                    self.show_delete_task_confirm = None;
                                    self.export_message = Some((format!("Task '{}' deleted", task_description), 3.0));
                                }
                                if no_button.clicked() || (no_button.has_focus() && (ui.input(|i| i.key_pressed(egui::Key::Enter)) || ui.input(|i| i.key_pressed(egui::Key::Escape)))) {
                                    self.show_delete_task_confirm = None;
                                }
                            });
                        });
                }
            }

            // Add the shortcuts popup window
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

                                ui.label("⌘S");
                                ui.label("Show Statistics");
                                ui.end_row();

                                ui.label("⌘,");
                                ui.label("Show Settings");
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

            // Add the settings popup window
            if self.show_settings {
                egui::Window::new("Settings")
                    .collapsible(false)
                    .resizable(false)
                    .show(ctx, |ui| {
                        ui.heading("UI Scale");
                        ui.add_space(4.0);

                        ui.horizontal(|ui| {
                            if ui.button("➖").clicked() && self.temporary_ui_scale > 1.0 {
                                self.temporary_ui_scale = (self.temporary_ui_scale - 0.1).max(1.0);
                            }

                            ui.add(
                                egui::Slider::new(&mut self.temporary_ui_scale, 1.0..=2.5)
                                    .step_by(0.1)
                                    .text("Scale"),
                            );

                            if ui.button("➕").clicked() && self.temporary_ui_scale < 2.5 {
                                self.temporary_ui_scale = (self.temporary_ui_scale + 0.1).min(2.5);
                            }
                        });

                        ui.add_space(8.0);
                        ui.horizontal(|ui| {
                            if ui.button("Revert to Default").clicked() {
                                self.temporary_ui_scale = 2.0;
                            }

                            ui.with_layout(
                                egui::Layout::right_to_left(egui::Align::Center),
                                |ui| {
                                    if ui.button("Close").clicked() {
                                        self.temporary_ui_scale = self.ui_scale; // Reset temporary scale
                                        self.show_settings = false;
                                    }
                                    if ui.button("Apply").clicked() {
                                        self.ui_scale = self.temporary_ui_scale;
                                        ctx.set_pixels_per_point(self.ui_scale);
                                    }
                                },
                            );
                        });
                    });
            }

            // Add the statistics window after the shortcuts window
            if self.show_statistics {
                egui::Window::new("Statistics")
                    .collapsible(false)
                    .resizable(true)
                    .default_size([400.0, 500.0])
                    .show(ctx, |ui| {
                        let content_height = ui.available_height() - 40.0; // Reserve space for close button

                        ui.horizontal(|ui| {
                            ui.selectable_value(&mut self.selected_stats_tab, StatsTab::Overview, "Overview");
                            ui.selectable_value(&mut self.selected_stats_tab, StatsTab::Projects, "Projects");
                            ui.selectable_value(&mut self.selected_stats_tab, StatsTab::Timeline, "Timeline");
                            ui.selectable_value(&mut self.selected_stats_tab, StatsTab::Details, "Details");
                        });
                        
                        ui.separator();

                        egui::ScrollArea::vertical()
                            .max_height(content_height)
                            .show(ui, |ui| {
                                match self.selected_stats_tab {
                                    StatsTab::Overview => {
                                        ui.heading("Overview");
                                        ui.add_space(8.0);
                                        
                                        // Filter tasks to only include those in existing folders or uncategorized
                                        let current_tasks: Vec<_> = self.tasks.values()
                                            .filter(|task| {
                                                match &task.folder {
                                                    None => true, // Include uncategorized tasks
                                                    Some(folder) => self.folders.contains(folder) // Only include tasks from existing folders
                                                }
                                            })
                                            .collect();
                                        
                                        // Total tracked time
                                        let total_time: i64 = current_tasks.iter()
                                            .map(|t| t.get_current_duration())
                                            .sum();
                                        ui.label(format!("Total Time Tracked: {}", Self::format_duration(total_time)));
                                        
                                        // Active tasks
                                        let active_tasks = current_tasks.iter()
                                            .filter(|t| t.start_time.is_some())
                                            .count();
                                        ui.label(format!("Currently Active Tasks: {}", active_tasks));
                                        
                                        // Average task duration
                                        let avg_duration = if !current_tasks.is_empty() {
                                            total_time / current_tasks.len() as i64
                                        } else {
                                            0
                                        };
                                        ui.label(format!("Average Task Duration: {}", Self::format_duration(avg_duration)));
                                        
                                        ui.add_space(16.0);
                                        
                                        // Quick stats grid
                                        egui::Grid::new("stats_grid")
                                            .num_columns(2)
                                            .spacing([40.0, 8.0])
                                            .show(ui, |ui| {
                                                ui.label("Total Projects:");
                                                ui.label(format!("{}", self.folders.len()));
                                                ui.end_row();
                                                
                                                ui.label("Total Tasks:");
                                                ui.label(format!("{}", current_tasks.len()));
                                                ui.end_row();
                                                
                                                ui.label("Completed Tasks:");
                                                ui.label(format!("{}", current_tasks.iter()
                                                    .filter(|t| t.total_duration > 0 && !t.is_paused && t.start_time.is_none())
                                                    .count()));
                                                ui.end_row();
                                            });
                                    },
                                    StatsTab::Projects => {
                                        ui.heading("Project Statistics");
                                        ui.add_space(8.0);
                                        
                                        // Project time distribution
                                        let folder_durations = self.calculate_folder_durations();
                                        
                                        // Skip rendering if no data
                                        if folder_durations.is_empty() {
                                            ui.label("No project data available");
                                            return;
                                        }
                                        
                                        let max_duration = folder_durations[0].1;
                                        if max_duration == 0 {
                                            ui.label("No time tracked in any projects");
                                            return;
                                        }
                                        
                                        // Use a fixed width for consistent layout
                                        let available_width = ui.available_width();
                                        let label_width = available_width * 0.3;
                                        let bar_width = available_width * 0.7;
                                        
                                        for (folder, duration) in folder_durations {
                                            ui.horizontal(|ui| {
                                                // Fixed width for the folder name
                                                ui.with_layout(egui::Layout::left_to_right(egui::Align::Center), |ui| {
                                                    ui.set_min_width(label_width);
                                                    ui.label(&folder);
                                                });
                                                
                                                // Fixed width for the progress bar
                                                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                                    ui.set_min_width(bar_width);
                                                    let progress = duration as f32 / max_duration as f32;
                                                    let bar = egui::ProgressBar::new(progress)
                                                        .text(Self::format_duration(duration))
                                                        .animate(false);  // Disable animation
                                                    ui.add(bar);
                                                });
                                            });
                                        }
                                    },
                                    StatsTab::Timeline => {
                                        ui.heading("Activity Timeline");
                                        ui.add_space(8.0);
                                        
                                        ui.label("Coming soon: Activity visualization");
                                        ui.add_space(8.0);
                                        ui.label("This tab will show your activity patterns over time,");
                                        ui.label("including daily and weekly summaries.");
                                    },
                                    StatsTab::Details => {
                                        ui.heading("Detailed Statistics");
                                        ui.add_space(8.0);
                                        
                                        // Most time-consuming tasks
                                        ui.label("Top Tasks by Duration:");
                                        ui.add_space(4.0);
                                        
                                        // Filter tasks to only include those in existing folders or uncategorized
                                        let mut tasks: Vec<_> = self.tasks.values()
                                            .filter(|task| {
                                                match &task.folder {
                                                    None => true, // Include uncategorized tasks
                                                    Some(folder) => self.folders.contains(folder) // Only include tasks from existing folders
                                                }
                                            })
                                            .collect();
                                        
                                        if tasks.is_empty() {
                                            ui.label(egui::RichText::new("No tasks available")
                                                .italics()
                                                .color(egui::Color32::from_rgb(128, 128, 128)));
                                            return;
                                        }
                                        
                                        tasks.sort_by_key(|t| std::cmp::Reverse(t.get_current_duration()));
                                        
                                        for task in tasks.iter().take(5) {
                                            ui.horizontal(|ui| {
                                                // Show folder name along with task description
                                                let folder_name = task.folder.as_deref().unwrap_or("Uncategorized");
                                                ui.label(format!("{} ({})", task.description, folder_name));
                                                
                                                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                                    ui.label(Self::format_duration(task.get_current_duration()));
                                                });
                                            });
                                        }
                                    }
                                }
                            });

                        // Always show close button at the bottom
                        ui.add_space(8.0);
                        ui.with_layout(egui::Layout::bottom_up(egui::Align::Center), |ui| {
                            if ui.button("Close").clicked() {
                                self.show_statistics = false;
                            }
                        });
                    });
            }

            ui.add_space(16.0);

            // Folder selection and creation
            ui.horizontal(|ui| {
                if ui.button("📁 New Folder").clicked() {
                    self.show_new_folder_dialog = true;
                    self.focus_new_folder = true;
                }
                if !self.folders.is_empty() {
                    if ui.button("🗑 Clear Folders").clicked() {
                        self.show_clear_folders_confirm = true;
                    }
                }
            });

            // Confirmation dialog for clearing all folders
            if self.show_clear_folders_confirm {
                egui::Window::new("Clear All Folders")
                    .collapsible(false)
                    .resizable(false)
                    .show(ctx, |ui| {
                        ui.label("Are you sure you want to clear all folders? This will remove all folder organization but keep your tasks. This cannot be undone.");
                        ui.horizontal(|ui| {
                            ui.spacing_mut().item_spacing.x = 10.0;
                            let yes_button = ui.add(egui::Button::new("Yes"));
                            let no_button = ui.add(egui::Button::new("No"));
                            
                            let dialog_id = ui.id().with("clear_folders_dialog");
                            let focus_id = dialog_id.with("focus");
                            
                            // Initialize focus to "yes" if not set
                            if !ui.memory(|mem| mem.data.get_temp::<bool>(focus_id).is_some()) {
                                ui.memory_mut(|mem| mem.data.insert_temp(focus_id, true));  // true = yes focused
                            }

                            let mut yes_focused = ui.memory(|mem| mem.data.get_temp::<bool>(focus_id).unwrap_or(true));

                            // Handle tab navigation
                            if ui.input(|i| i.key_pressed(egui::Key::Tab)) {
                                yes_focused = !yes_focused;
                                ui.memory_mut(|mem| mem.data.insert_temp(focus_id, yes_focused));
                            }

                            // Apply focus based on memory state
                            if yes_focused {
                                yes_button.request_focus();
                            } else {
                                no_button.request_focus();
                            }

                            if yes_button.clicked() || (yes_button.has_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter))) {
                                self.clear_all_folders();
                                self.show_clear_folders_confirm = false;
                                self.export_message = Some(("All folders cleared".to_string(), 3.0));
                            }
                            if no_button.clicked() || (no_button.has_focus() && (ui.input(|i| i.key_pressed(egui::Key::Enter)) || ui.input(|i| i.key_pressed(egui::Key::Escape)))) {
                                self.show_clear_folders_confirm = false;
                            }
                        });
                    });
            }

            // New folder dialog
            if self.show_new_folder_dialog {
                egui::Window::new("New Folder")
                    .collapsible(false)
                    .resizable(false)
                    .show(ctx, |ui| {
                        ui.horizontal(|ui| {
                            let text_edit = ui.text_edit_singleline(&mut self.new_folder_input);
                            let create_button = ui.button("Create");
                            let cancel_button = ui.button("Cancel");
                            
                            let dialog_id = ui.id().with("new_folder_dialog");
                            let focus_id = dialog_id.with("focus");
                            
                            // Initialize focus state to text input (0) only when dialog opens
                            if !ui.memory(|mem| mem.data.get_temp::<u8>(focus_id).is_some()) {
                                ui.memory_mut(|mem| mem.data.insert_temp(focus_id, 0));
                                text_edit.request_focus();
                            }

                            let mut focus_state = ui.memory(|mem| mem.data.get_temp::<u8>(focus_id).unwrap_or(0));

                            // Handle tab navigation
                            if ui.input(|i| i.key_pressed(egui::Key::Tab)) {
                                if ui.input(|i| i.modifiers.shift) {
                                    // Shift+Tab goes backwards
                                    focus_state = if focus_state == 0 { 2 } else { focus_state - 1 };
                                } else {
                                    // Tab goes forwards
                                    focus_state = if focus_state == 2 { 0 } else { focus_state + 1 };
                                }
                                ui.memory_mut(|mem| mem.data.insert_temp(focus_id, focus_state));
                            }

                            // Apply focus based on state
                            match focus_state {
                                0 => text_edit.request_focus(),
                                1 => create_button.request_focus(),
                                2 => cancel_button.request_focus(),
                                _ => {}
                            }

                            let enter_pressed = ui.input(|i| i.key_pressed(egui::Key::Enter));
                            
                            let mut should_close = false;
                            
                            if (create_button.clicked() || (enter_pressed && focus_state == 1))
                                && !self.new_folder_input.trim().is_empty()
                            {
                                self.add_folder(self.new_folder_input.trim().to_string());
                                self.new_folder_input.clear();
                                should_close = true;
                            }
                            
                            // Only create folder from text input if Enter is pressed while focused
                            if enter_pressed && focus_state == 0 && !self.new_folder_input.trim().is_empty() {
                                self.add_folder(self.new_folder_input.trim().to_string());
                                self.new_folder_input.clear();
                                should_close = true;
                            }
                            
                            if cancel_button.clicked() || (enter_pressed && focus_state == 2) || ui.input(|i| i.key_pressed(egui::Key::Escape)) {
                                should_close = true;
                            }

                            if should_close {
                                // Clear focus state from memory when closing
                                ui.memory_mut(|mem| mem.data.remove::<u8>(focus_id));
                                self.show_new_folder_dialog = false;
                                self.new_folder_input.clear();
                            }
                        });
                    });
            }

            ui.add_space(16.0);

            // Display tasks by folder with custom colors
            egui::ScrollArea::vertical().show(ui, |ui| {
                let folders = self.get_folders();
                let tasks_by_folder = self.get_tasks_by_folder();

                // Add a drop target at the top of the list
                if let Some(dragged_folder) = &self.dragged_folder {
                    let top_rect = ui.available_rect_before_wrap();
                    let top_indicator_rect = egui::Rect::from_min_max(
                        top_rect.left_top(),
                        top_rect.right_top() + egui::vec2(0.0, 4.0),
                    );

                    let response = ui.allocate_rect(top_indicator_rect, egui::Sense::hover());
                    if response.hovered() {
                        // Show insertion indicator at the top
                        ui.painter().rect_filled(
                            top_indicator_rect,
                            0.0,
                            ui.visuals().selection.stroke.color,
                        );

                        // Handle dropping at the top
                        if ui.input(|i| i.pointer.any_released()) {
                            if let Some(src_idx) = self.folders.iter().position(|f| f == dragged_folder) {
                                let folder = self.folders.remove(src_idx);
                                self.folders.insert(0, folder);
                                if self.focused_folder_index == Some(src_idx) {
                                    self.focused_folder_index = Some(0);
                                }
                                self.save_tasks();
                            }
                            self.dragged_folder = None;
                        }
                    }
                }

                for (folder_idx, folder) in folders.iter().enumerate() {
                    let folder_name = folder.clone();
                    let task_ids = tasks_by_folder.get(folder_name.as_str()).cloned().unwrap_or_default();

                    egui::Frame::new()
                        .outer_margin(egui::Vec2::splat(2.0))
                        .show(ui, |ui| {
                            let folder_id = egui::Id::new(format!("folder_{}", folder_name));
                            let mut is_open = ui.memory_mut(|mem| {
                                mem.data.get_temp::<bool>(folder_id).unwrap_or(true)
                            });

                            // Handle left/right arrow keys for the focused folder
                            if Some(folder_idx) == self.focused_folder_index {
                                if ctx.input(|i| i.key_pressed(egui::Key::ArrowRight)) && !is_open {
                                    is_open = true;
                                    ui.memory_mut(|mem| {
                                        mem.data.insert_temp(folder_id, true);
                                    });
                                }
                                if ctx.input(|i| i.key_pressed(egui::Key::ArrowLeft)) && is_open {
                                    is_open = false;
                                    ui.memory_mut(|mem| {
                                        mem.data.insert_temp(folder_id, false);
                                    });
                                }
                            }

                            // Header row with folder name and buttons
                            ui.horizontal(|ui| {
                                ui.spacing_mut().item_spacing.x = 10.0;

                                // Create a draggable button that contains the folder name and arrow
                                let arrow = if is_open { fill::CARET_DOWN } else { fill::CARET_RIGHT };
                                
                                // Add visual feedback for focused folder
                                let mut button = egui::Button::new(format!("{} {} ({})", arrow, folder_name, task_ids.len()))
                                    .sense(egui::Sense::click_and_drag());
                                
                                if Some(folder_idx) == self.focused_folder_index {
                                    button = button.fill(ui.visuals().selection.bg_fill);
                                }
                                
                                let folder_button = ui.add(button);

                                // Handle drag and drop
                                if folder_button.drag_started() {
                                    self.dragged_folder = Some(folder_name.clone());
                                }
                                
                                if let Some(dragged_folder) = &self.dragged_folder {
                                    if folder_button.dragged() {
                                        // Show drag preview with improved visual feedback
                                        let rect = folder_button.rect.expand(2.0);
                                        ui.painter().rect_stroke(
                                            rect,
                                            0.0,
                                            egui::Stroke::new(2.0, ui.visuals().selection.stroke.color),
                                            egui::epaint::StrokeKind::Inside,
                                        );
                                    }
                                    
                                    // Only show drop indicators if we're not dragging the current folder
                                    if dragged_folder != &folder_name {
                                        let src_idx = self.folders.iter().position(|f| f == dragged_folder);
                                        let hover_rect = folder_button.rect.expand(4.0);
                                        
                                        if ui.rect_contains_pointer(hover_rect) {
                                            let is_below = ui.input(|i| {
                                                i.pointer.hover_pos().map_or(false, |pos| pos.y > folder_button.rect.center().y)
                                            });
                                            
                                            // Only show indicator if dropping would result in a meaningful reorder
                                            let should_show_indicator = if let Some(src_idx) = src_idx {
                                                if is_below {
                                                    // When dropping below, only show if source is above this folder
                                                    src_idx < folder_idx
                                                } else {
                                                    // When dropping above, only show if source is below this folder
                                                    src_idx > folder_idx
                                                }
                                            } else {
                                                false
                                            };
                                            
                                            if should_show_indicator {
                                                let indicator_rect = if is_below {
                                                    egui::Rect::from_min_max(
                                                        folder_button.rect.left_bottom() + egui::vec2(0.0, 2.0),
                                                        folder_button.rect.right_bottom() + egui::vec2(0.0, 4.0),
                                                    )
                                                } else {
                                                    egui::Rect::from_min_max(
                                                        folder_button.rect.left_top() - egui::vec2(0.0, 4.0),
                                                        folder_button.rect.right_top() - egui::vec2(0.0, 2.0),
                                                    )
                                                };
                                                
                                                ui.painter().rect_filled(
                                                    indicator_rect,
                                                    0.0,
                                                    ui.visuals().selection.stroke.color,
                                                );
                                                
                                                // Handle dropping near a folder
                                                if ui.input(|i| i.pointer.any_released()) {
                                                    if let Some(src_idx) = src_idx {
                                                        let folder = self.folders.remove(src_idx);
                                                        let insert_idx = if is_below {
                                                            (folder_idx + 1).min(self.folders.len())
                                                        } else {
                                                            folder_idx
                                                        };
                                                        self.folders.insert(insert_idx, folder);
                                                        if self.focused_folder_index == Some(src_idx) {
                                                            self.focused_folder_index = Some(insert_idx);
                                                        }
                                                        self.save_tasks();
                                                    }
                                                    self.dragged_folder = None;
                                                }
                                            }
                                        }
                                    }
                                }

                                if folder_button.clicked() {
                                    is_open = !is_open;
                                    ui.memory_mut(|mem| {
                                        mem.data.insert_temp(folder_id, is_open);
                                    });
                                }

                                // Right side: Export and Clear buttons
                                ui.with_layout(
                                    egui::Layout::right_to_left(egui::Align::Center),
                                    |ui| {
                                        if ui.button("🗑").clicked() {
                                            self.show_clear_folder_confirm = Some(folder_name.clone());
                                        }
                                        ui.small("Clear");

                                        ui.separator();

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

                                        ui.separator();

                                        if ui.button("➕").clicked() {
                                            self.show_add_task_dialog = true;
                                            self.add_task_to_folder = Some(folder_name.clone());
                                            self.new_task_in_folder.clear();
                                        }
                                        ui.small("Add Task");
                                    },
                                );
                            });

                            // Collapsible content
                            if is_open {
                                ui.indent("tasks", |ui| {
                                    if task_ids.is_empty() {
                                        ui.add_space(4.0);
                                        ui.label(egui::RichText::new("No tasks in this folder")
                                            .italics()
                                            .color(egui::Color32::from_rgb(128, 128, 128)));
                                    } else {
                                        // Display tasks in the folder
                                        let mut task_action = None;
                                        let mut task_action_id = None;
                                        let mut task_export_error = None;

                                        for (task_idx, task_id) in task_ids.iter().enumerate() {
                                            if let Some(task) = self.tasks.get(task_id) {
                                                let is_focused = Some(folder_idx) == self.focused_folder_index && 
                                                              Some(task_idx) == self.focused_task_index;
                                                
                                                // Add a frame around the task if it's focused
                                                let task_frame = egui::Frame::new()
                                                    .fill(if is_focused { 
                                                        ui.visuals().selection.bg_fill 
                                                    } else { 
                                                        egui::Color32::TRANSPARENT 
                                                    });

                                                task_frame.show(ui, |ui| {
                                                    let (action, export_error) =
                                                        self.display_task(ui, task_id, task);
                                                    if action.is_some() {
                                                        task_action = action;
                                                        task_action_id = Some(task_id.to_string());
                                                    }
                                                    if export_error.is_some() {
                                                        task_export_error = export_error;
                                                    }
                                                });
                                            }
                                        }

                                        // Handle any actions outside the closure
                                        if let Some(action) = task_action {
                                            if let Some(id) = task_action_id {
                                                self.handle_task_action(&id, action);
                                            }
                                        }
                                        if let Some(error) = task_export_error {
                                            self.export_message = Some((error, 3.0));
                                        }
                                    }
                                });
                            }
                        });
                }
            });

            // Add task dialog
            if self.show_add_task_dialog {
                if let Some(folder_name) = &self.add_task_to_folder {
                    let mut should_close = false;
                    let mut should_add_task = false;
                    let folder_name = folder_name.clone();

                    egui::Window::new(format!("Add Task to '{}'", folder_name))
                        .collapsible(false)
                        .resizable(false)
                        .show(ctx, |ui| {
                            ui.horizontal(|ui| {
                                let text_edit = ui.text_edit_singleline(&mut self.new_task_in_folder);
                                let add_button = ui.button("Add");
                                let cancel_button = ui.button("Cancel");
                                
                                let dialog_id = ui.id().with("add_task_dialog");
                                let focus_id = dialog_id.with("focus");
                                
                                // Initialize focus state to text input (0) when dialog opens
                                if !ui.memory(|mem| mem.data.get_temp::<u8>(focus_id).is_some()) {
                                    ui.memory_mut(|mem| mem.data.insert_temp(focus_id, 0));
                                    text_edit.request_focus();
                                }

                                let mut focus_state = ui.memory(|mem| mem.data.get_temp::<u8>(focus_id).unwrap_or(0));

                                // Handle tab navigation
                                if ui.input(|i| i.key_pressed(egui::Key::Tab)) {
                                    if ui.input(|i| i.modifiers.shift) {
                                        // Shift+Tab goes backwards
                                        focus_state = if focus_state == 0 { 2 } else { focus_state - 1 };
                                    } else {
                                        // Tab goes forwards
                                        focus_state = if focus_state == 2 { 0 } else { focus_state + 1 };
                                    }
                                    ui.memory_mut(|mem| mem.data.insert_temp(focus_id, focus_state));
                                }

                                // Apply focus based on state
                                match focus_state {
                                    0 => text_edit.request_focus(),
                                    1 => add_button.request_focus(),
                                    2 => cancel_button.request_focus(),
                                    _ => {}
                                }

                                let enter_pressed = ui.input(|i| i.key_pressed(egui::Key::Enter));

                                if (add_button.clicked() || (enter_pressed && focus_state == 1))
                                    && !self.new_task_in_folder.trim().is_empty()
                                {
                                    should_add_task = true;
                                    should_close = true;
                                }

                                if enter_pressed && focus_state == 0 && !self.new_task_in_folder.trim().is_empty() {
                                    should_add_task = true;
                                    should_close = true;
                                }

                                if cancel_button.clicked() || (enter_pressed && focus_state == 2) || ui.input(|i| i.key_pressed(egui::Key::Escape)) {
                                    should_close = true;
                                }

                                if should_close {
                                    ui.memory_mut(|mem| mem.data.remove::<u8>(focus_id));
                                }
                            });
                        });

                    if should_add_task {
                        let mut task = Task::new(self.new_task_in_folder.trim().to_string());
                        task.folder = Some(folder_name);
                        self.tasks.insert(task.id.clone(), task);
                        self.save_tasks();
                    }

                    if should_close {
                        self.show_add_task_dialog = false;
                        self.add_task_to_folder = None;
                        self.new_task_in_folder.clear();
                    }
                }
            }
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
        Box::new(|cc| {
            // Load both regular and fill Phosphor icons fonts
            let mut fonts = egui::FontDefinitions::default();
            egui_phosphor::add_to_fonts(&mut fonts, egui_phosphor::Variant::Regular);
            egui_phosphor::add_to_fonts(&mut fonts, egui_phosphor::Variant::Fill);
            cc.egui_ctx.set_fonts(fonts);
            
            Ok(Box::new(WorkTimer::new()) as Box<dyn eframe::App>)
        }),
    )
}
