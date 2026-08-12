pub trait ChartPopoutController: Send + Sync {
    fn set_enabled(&mut self, enabled: bool);
    fn set_opacity(&mut self, opacity: f32);
    fn set_click_through(&mut self, click_through: bool);
    fn set_always_on_top(&mut self, always_on_top: bool);
    fn update_from_input(&mut self, ctrl_down: bool, hovered: bool, configured_opacity: f32);
}

#[cfg(windows)]
#[derive(Debug, Clone)]
pub struct WindowsChartPopoutController {
    enabled: bool,
    opacity: f32,
    click_through: bool,
    always_on_top: bool,
}

#[cfg(windows)]
impl Default for WindowsChartPopoutController {
    fn default() -> Self {
        Self {
            enabled: false,
            opacity: 0.8,
            click_through: false,
            always_on_top: true,
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
        self.click_through = enabled && !self.always_on_top;
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
}

#[cfg(not(windows))]
#[derive(Debug, Clone)]
pub struct LinuxChartPopoutController {
    enabled: bool,
    opacity: f32,
    click_through: bool,
    always_on_top: bool,
}

#[cfg(not(windows))]
impl Default for LinuxChartPopoutController {
    fn default() -> Self {
        Self {
            enabled: false,
            opacity: 0.8,
            click_through: false,
            always_on_top: true,
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
}

#[cfg(windows)]
pub fn create_chart_popout_controller() -> Box<dyn ChartPopoutController> {
    Box::new(WindowsChartPopoutController::new())
}

#[cfg(not(windows))]
pub fn create_chart_popout_controller() -> Box<dyn ChartPopoutController> {
    Box::new(LinuxChartPopoutController::new())
}
