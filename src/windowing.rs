use eframe::egui;

pub trait ChartPopoutController: Send + Sync {
    fn set_enabled(&mut self, enabled: bool);
    fn set_opacity(&mut self, opacity: f32);
    fn set_click_through(&mut self, click_through: bool);
    fn set_always_on_top(&mut self, always_on_top: bool);
    fn set_viewport_id(&mut self, viewport_id: egui::ViewportId);
    fn update_from_input(&mut self, ctrl_down: bool, hovered: bool, configured_opacity: f32);
    fn apply_platform_state(
        &mut self,
        ctx: &egui::Context,
        viewport_id: egui::ViewportId,
        ctrl_down: bool,
        hovered: bool,
        configured_opacity: f32,
    );
}

#[cfg(windows)]
#[derive(Debug, Clone)]
pub struct WindowsChartPopoutController {
    enabled: bool,
    opacity: f32,
    click_through: bool,
    always_on_top: bool,
    viewport_id: Option<egui::ViewportId>,
}

#[cfg(windows)]
impl Default for WindowsChartPopoutController {
    fn default() -> Self {
        Self {
            enabled: false,
            opacity: 0.8,
            click_through: false,
            always_on_top: true,
            viewport_id: None,
        }
    }
}

#[cfg(windows)]
impl WindowsChartPopoutController {
    pub fn new() -> Self {
        Self::default()
    }
}

#[cfg(windows)]
impl ChartPopoutController for WindowsChartPopoutController {
    fn set_enabled(&mut self, enabled: bool) {
        self.enabled = enabled;
    }

    fn set_opacity(&mut self, opacity: f32) {
        self.opacity = opacity.clamp(0.0, 1.0);
    }

    fn set_click_through(&mut self, click_through: bool) {
        self.click_through = click_through;
    }

    fn set_always_on_top(&mut self, always_on_top: bool) {
        self.always_on_top = always_on_top;
    }

    fn set_viewport_id(&mut self, viewport_id: egui::ViewportId) {
        self.viewport_id = Some(viewport_id);
    }

    fn update_from_input(&mut self, ctrl_down: bool, hovered: bool, configured_opacity: f32) {
        if !self.enabled {
            return;
        }

        let opacity = if ctrl_down || hovered {
            1.0
        } else {
            configured_opacity.clamp(0.0, 1.0)
        };

        self.opacity = opacity;
        self.click_through = !ctrl_down && !hovered;
    }

    fn apply_platform_state(
        &mut self,
        ctx: &egui::Context,
        viewport_id: egui::ViewportId,
        ctrl_down: bool,
        hovered: bool,
        configured_opacity: f32,
    ) {
        self.viewport_id = Some(viewport_id);

        if !self.enabled {
            ctx.send_viewport_cmd_to(viewport_id, egui::ViewportCommand::Visible(false));
            ctx.send_viewport_cmd_to(viewport_id, egui::ViewportCommand::Transparent(false));
            ctx.send_viewport_cmd_to(viewport_id, egui::ViewportCommand::MousePassthrough(false));
            ctx.send_viewport_cmd_to(
                viewport_id,
                egui::ViewportCommand::WindowLevel(egui::viewport::WindowLevel::Normal),
            );
            ctx.send_viewport_cmd_to(viewport_id, egui::ViewportCommand::Decorations(false));
            ctx.send_viewport_cmd_to(viewport_id, egui::ViewportCommand::Resizable(true));
            return;
        }

        self.update_from_input(ctrl_down, hovered, configured_opacity);

        ctx.send_viewport_cmd_to(viewport_id, egui::ViewportCommand::Visible(true));
        ctx.send_viewport_cmd_to(viewport_id, egui::ViewportCommand::Transparent(false));
        ctx.send_viewport_cmd_to(viewport_id, egui::ViewportCommand::MousePassthrough(false));
        ctx.send_viewport_cmd_to(
            viewport_id,
            egui::ViewportCommand::WindowLevel(egui::viewport::WindowLevel::AlwaysOnTop),
        );
        ctx.send_viewport_cmd_to(viewport_id, egui::ViewportCommand::Decorations(false));
        ctx.send_viewport_cmd_to(viewport_id, egui::ViewportCommand::Resizable(true));
        self.click_through = false;
        ctx.request_repaint_of(viewport_id);
    }
}

#[cfg(not(windows))]
#[derive(Debug, Clone)]
pub struct LinuxChartPopoutController {
    enabled: bool,
    opacity: f32,
    click_through: bool,
    always_on_top: bool,
    viewport_id: Option<egui::ViewportId>,
}

#[cfg(not(windows))]
impl Default for LinuxChartPopoutController {
    fn default() -> Self {
        Self {
            enabled: false,
            opacity: 0.8,
            click_through: false,
            always_on_top: true,
            viewport_id: None,
        }
    }
}

#[cfg(not(windows))]
impl LinuxChartPopoutController {
    pub fn new() -> Self {
        Self::default()
    }
}

#[cfg(not(windows))]
impl ChartPopoutController for LinuxChartPopoutController {
    fn set_enabled(&mut self, enabled: bool) {
        self.enabled = enabled;
        self.click_through = enabled;
    }

    fn set_opacity(&mut self, opacity: f32) {
        self.opacity = opacity.clamp(0.0, 1.0);
    }

    fn set_click_through(&mut self, click_through: bool) {
        self.click_through = click_through;
    }

    fn set_always_on_top(&mut self, always_on_top: bool) {
        self.always_on_top = always_on_top;
    }

    fn set_viewport_id(&mut self, viewport_id: egui::ViewportId) {
        self.viewport_id = Some(viewport_id);
    }

    fn update_from_input(&mut self, ctrl_down: bool, hovered: bool, configured_opacity: f32) {
        if !self.enabled {
            return;
        }

        let opacity = if ctrl_down || hovered {
            1.0
        } else {
            configured_opacity.clamp(0.0, 1.0)
        };

        self.opacity = opacity;
        self.click_through = !ctrl_down && !hovered;
    }

    fn apply_platform_state(
        &mut self,
        _ctx: &egui::Context,
        _viewport_id: egui::ViewportId,
        _ctrl_down: bool,
        _hovered: bool,
        _configured_opacity: f32,
    ) {
        // Placeholder for future Linux implementation. The shared API keeps the
        // Windows path first while leaving a clean hook for X11/Wayland later.
    }
}

#[cfg(windows)]
pub fn create_chart_popout_controller() -> Box<dyn ChartPopoutController> {
    Box::new(WindowsChartPopoutController::new())
}

#[cfg(not(windows))]
pub fn create_chart_popout_controller() -> Box<dyn ChartPopoutController> {
    Box::new(LinuxChartPopoutController::new())
}
