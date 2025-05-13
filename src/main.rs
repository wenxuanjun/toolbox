#![windows_subsystem = "windows"]

use eframe::egui::IconData;
use eframe::{egui, CreationContext, NativeOptions};
use egui::FontFamily::Proportional;
use egui::{CentralPanel, SidePanel, TopBottomPanel, ViewportBuilder};
use egui::{Context, RichText, ScrollArea, Ui};
use egui::{FontData, FontDefinitions, FontFamily};
use egui::{FontId, TextStyle::*};
use std::os::windows::ffi::OsStrExt;
use std::path::Path;
use std::sync::mpsc::{channel, Receiver, Sender};
use std::thread;
use std::{fs, ptr};
use winapi::um::shellapi::ShellExecuteW;
use winapi::um::winuser::SW_SHOW;

mod tools_info;
use tools_info::{Tool, CATEGORIES, TOOLS};

struct ToolboxApp {
    categories: &'static [&'static str],
    tools: &'static [Tool<'static>],
    selected_category: usize,
    search_query: String,
    launch_sender: Sender<usize>,
    launch_receiver: Receiver<usize>,
    error_popup: Option<String>,
    tools_version: String,
}

impl ToolboxApp {
    fn new(cc: &CreationContext<'_>) -> Self {
        let (sender, receiver) = channel();

        let mut style = (*cc.egui_ctx.style()).clone();

        style.text_styles = [
            (Heading, FontId::new(18.0, Proportional)),
            (Body, FontId::new(14.0, Proportional)),
            (Button, FontId::new(14.0, Proportional)),
        ]
        .into();

        cc.egui_ctx.set_style(style);

        let mut fonts = FontDefinitions::default();

        let font_path = Path::new("C:\\Windows\\Fonts\\msyh.ttc");
        if let Ok(font_data) = fs::read(font_path) {
            fonts.font_data.insert(
                "microsoft_yahei".to_owned(),
                FontData::from_owned(font_data).into(),
            );

            fonts
                .families
                .entry(FontFamily::Proportional)
                .or_default()
                .insert(0, "microsoft_yahei".to_owned());

            cc.egui_ctx.set_fonts(fonts);
        }

        // open tools/Version
        let version_path = Path::new("tools/Version");
        let tools_version = if version_path.exists() {
            fs::read_to_string(version_path).unwrap_or_else(|_| "未知".to_string())
        } else {
            "未知".to_string()
        };

        Self {
            categories: CATEGORIES,
            tools: TOOLS,
            selected_category: 0,
            search_query: String::new(),
            launch_sender: sender,
            launch_receiver: receiver,
            error_popup: None,
            tools_version,
        }
    }

    fn launch_tool(&mut self, index: usize) {
        if index >= self.tools.len() {
            return;
        }

        let tool = &self.tools[index];
        let path = Path::new(&tool.path);

        if !path.exists() {
            let message = format!("找不到工具：{}\n路径：{}", tool.name, tool.path);
            self.error_popup = Some(message);
            return;
        }

        let path_wide = path.as_os_str().encode_wide();
        let path_wide: Vec<u16> = path_wide.chain(Some(0)).collect();
        let operation: Vec<u16> = "open".encode_utf16().chain(Some(0)).collect();

        unsafe {
            ShellExecuteW(
                ptr::null_mut(),
                operation.as_ptr(),
                path_wide.as_ptr(),
                ptr::null(),
                ptr::null(),
                SW_SHOW,
            );
        }
    }

    fn show_error_popup(&mut self, ctx: &Context) {
        if let Some(error_msg) = &self.error_popup {
            let mut open = true;

            egui::Window::new("错误")
                .collapsible(false)
                .resizable(false)
                .movable(true)
                .open(&mut open)
                .show(ctx, |ui| ui.label(error_msg));

            if !open {
                self.error_popup = None;
            }
        }
    }

    fn show_tools_ui(&mut self, ui: &mut Ui) {
        let category_name = &self.categories[self.selected_category];

        ui.heading(format!("分类: {category_name}"));
        ui.separator();

        let filtered_tools: Vec<(usize, &Tool)> = self
            .tools
            .iter()
            .enumerate()
            .filter(|(_, tool)| {
                if self.search_query.is_empty() {
                    tool.category == *category_name
                } else {
                    tool.name
                        .to_lowercase()
                        .contains(&self.search_query.to_lowercase())
                }
            })
            .collect();

        ScrollArea::vertical()
            .auto_shrink([false, false])
            .show(ui, |ui| {
                if filtered_tools.is_empty() {
                    ui.label("没有找到工具");
                    return;
                }

                let available_width = ui.available_width();
                let (button_width, button_height, spacing) = (100.0, 80.0, 10.0);
                let buttons_per_row =
                    ((available_width / (button_width + spacing)).floor() as usize).max(1);

                let grid = egui::Grid::new("tools_grid")
                    .spacing([spacing, spacing])
                    .min_col_width(button_width)
                    .max_col_width(button_width);

                grid.show(ui, |ui| {
                    for (grid_pos, (index, tool)) in filtered_tools.iter().enumerate() {
                        if grid_pos > 0 && grid_pos % buttons_per_row == 0 {
                            ui.end_row();
                        }

                        let button = ui.add_sized(
                            [button_width, button_height],
                            egui::Button::new(RichText::new(tool.name).size(14.0)).wrap(),
                        );

                        if button.clicked() {
                            let sender = self.launch_sender.clone();
                            let tool_index = *index;
                            thread::spawn(move || {
                                let _ = sender.send(tool_index);
                            });
                        }

                        if button.hovered() {
                            let strip_path = tool
                                .path
                                .strip_prefix(".\\tools\\")
                                .unwrap_or(&tool.path)
                                .replace("\\", "/");

                            egui::show_tooltip(
                                ui.ctx(),
                                ui.layer_id(),
                                egui::Id::new("tool_tooltip"),
                                |ui| {
                                    ui.set_max_width(300.0);
                                    ui.label(strip_path);
                                },
                            );
                        }
                    }
                });
            });
    }
}

impl eframe::App for ToolboxApp {
    fn update(&mut self, ctx: &Context, _frame: &mut eframe::Frame) {
        if let Ok(tool_index) = self.launch_receiver.try_recv() {
            self.launch_tool(tool_index);
        }

        TopBottomPanel::top("top_panel").show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.label(format!("版本: {}", self.tools_version));
                ui.add_space(4.0);
                ui.separator();
                ui.add_space(4.0);

                ui.label("搜索:");
                ui.text_edit_singleline(&mut self.search_query);
            });
        });

        SidePanel::left("categories_panel")
            .width_range(150.0..=300.0)
            .show(ctx, |ui| {
                ui.vertical(|ui| {
                    ScrollArea::vertical()
                        .auto_shrink([false, true])
                        .show(ui, |ui| {
                            for (idx, category) in self.categories.iter().enumerate() {
                                if ui
                                    .selectable_label(idx == self.selected_category, *category)
                                    .clicked()
                                {
                                    self.selected_category = idx;
                                }
                            }
                        });
                });
            });

        CentralPanel::default().show(ctx, |ui| {
            self.show_tools_ui(ui);
        });

        self.show_error_popup(ctx);
    }
}

fn main() -> Result<(), eframe::Error> {
    let options = NativeOptions {
        viewport: ViewportBuilder::default()
            .with_icon(IconData::default())
            .with_inner_size([1000.0, 650.0]),
        centered: true,
        ..Default::default()
    };

    eframe::run_native(
        "图吧工具箱",
        options,
        Box::new(|cc| Ok(Box::new(ToolboxApp::new(cc)))),
    )
}
