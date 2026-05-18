// v0.4: capscr's plugin runtime arrives in v0.4. The `runtime` feature flips
// the `Plugin` trait impl + the path dep to `capscr` (Cargo.toml). Standalone
// builds compile as reference code.
#![cfg_attr(not(feature = "runtime"), allow(dead_code))]

#[cfg(feature = "runtime")]
use capscr::plugin::{Plugin, PluginEvent, PluginResponse};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HotbarConfig {
    pub enabled: bool,
    pub position: HotbarPosition,
    pub auto_hide: bool,
    pub auto_hide_delay_ms: u32,
    pub buttons: Vec<HotbarButton>,
    pub size: HotbarSize,
    pub theme: HotbarTheme,
    pub glass: GlassConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GlassConfig {
    pub enabled: bool,
    pub blur_amount: u32,
    pub tint_color: [u8; 4],
}

impl Default for GlassConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            blur_amount: 30,
            tint_color: [30, 30, 30, 180],
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HotbarPosition {
    Top,
    Bottom,
    Left,
    Right,
    TopLeft,
    TopRight,
    BottomLeft,
    BottomRight,
    Custom { x: i32, y: i32 },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HotbarButton {
    pub action: HotbarAction,
    pub label: String,
    pub tooltip: Option<String>,
    pub hotkey: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HotbarAction {
    CaptureScreen,
    CaptureWindow,
    CaptureRegion,
    RecordGif,
    OpenSettings,
    ToggleHotbar,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HotbarSize {
    pub button_width: u32,
    pub button_height: u32,
    pub spacing: u32,
    pub padding: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HotbarTheme {
    pub background: [u8; 4],
    pub button_background: [u8; 4],
    pub button_hover: [u8; 4],
    pub button_active: [u8; 4],
    pub text: [u8; 4],
    pub border: [u8; 4],
    pub border_radius: u32,
}

impl Default for HotbarConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            position: HotbarPosition::Bottom,
            auto_hide: false,
            auto_hide_delay_ms: 2000,
            buttons: vec![
                HotbarButton {
                    action: HotbarAction::CaptureScreen,
                    label: "S".to_string(),
                    tooltip: Some("Capture Screen".to_string()),
                    hotkey: Some("Ctrl+Shift+S".to_string()),
                },
                HotbarButton {
                    action: HotbarAction::CaptureWindow,
                    label: "W".to_string(),
                    tooltip: Some("Capture Window".to_string()),
                    hotkey: Some("Ctrl+Shift+W".to_string()),
                },
                HotbarButton {
                    action: HotbarAction::CaptureRegion,
                    label: "R".to_string(),
                    tooltip: Some("Capture Region".to_string()),
                    hotkey: Some("Ctrl+Shift+R".to_string()),
                },
                HotbarButton {
                    action: HotbarAction::RecordGif,
                    label: "G".to_string(),
                    tooltip: Some("Record GIF".to_string()),
                    hotkey: Some("Ctrl+Shift+G".to_string()),
                },
            ],
            size: HotbarSize {
                button_width: 36,
                button_height: 36,
                spacing: 6,
                padding: 10,
            },
            theme: HotbarTheme {
                background: [0, 0, 0, 1],
                button_background: [255, 255, 255, 30],
                button_hover: [255, 255, 255, 50],
                button_active: [255, 255, 255, 70],
                text: [255, 255, 255, 255],
                border: [255, 255, 255, 40],
                border_radius: 8,
            },
            glass: GlassConfig::default(),
        }
    }
}

pub struct HotbarPlugin {
    config: HotbarConfig,
    config_path: PathBuf,
    running: Arc<AtomicBool>,
    window_thread: Option<thread::JoinHandle<()>>,
}

impl HotbarPlugin {
    pub fn new() -> Self {
        let config_path = Self::default_config_path();
        let config = Self::load_config(&config_path).unwrap_or_default();
        Self {
            config,
            config_path,
            running: Arc::new(AtomicBool::new(false)),
            window_thread: None,
        }
    }

    pub fn with_config(config: HotbarConfig) -> Self {
        Self {
            config,
            config_path: Self::default_config_path(),
            running: Arc::new(AtomicBool::new(false)),
            window_thread: None,
        }
    }

    fn default_config_path() -> PathBuf {
        directories::ProjectDirs::from("", "", "capscr")
            .map(|dirs| dirs.config_dir().join("plugins").join("hotbar.toml"))
            .unwrap_or_else(|| PathBuf::from("hotbar.toml"))
    }

    fn load_config(path: &PathBuf) -> Option<HotbarConfig> {
        let content = std::fs::read_to_string(path).ok()?;
        toml::from_str(&content).ok()
    }

    fn save_config(&self) {
        if let Some(parent) = self.config_path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        if let Ok(content) = toml::to_string_pretty(&self.config) {
            let _ = std::fs::write(&self.config_path, content);
        }
    }

    #[cfg(windows)]
    fn start_hotbar(&mut self) {
        if !self.config.enabled {
            return;
        }

        self.running.store(true, Ordering::SeqCst);
        let running = Arc::clone(&self.running);
        let config = self.config.clone();

        let handle = thread::spawn(move || {
            let _ = run_hotbar_window(config, running);
        });

        self.window_thread = Some(handle);
    }

    #[cfg(not(windows))]
    fn start_hotbar(&mut self) {}

    fn stop_hotbar(&mut self) {
        self.running.store(false, Ordering::SeqCst);
        if let Some(handle) = self.window_thread.take() {
            let _ = handle.join();
        }
    }
}

#[cfg(windows)]
fn run_hotbar_window(config: HotbarConfig, running: Arc<AtomicBool>) -> Result<(), ()> {
    use std::mem::zeroed;
    use windows::core::PCWSTR;
    use windows::Win32::Foundation::HINSTANCE;
    use windows::Win32::Graphics::Dwm::DwmExtendFrameIntoClientArea;
    use windows::Win32::System::LibraryLoader::GetModuleHandleW;
    use windows::Win32::UI::Controls::MARGINS;
    use windows::Win32::UI::WindowsAndMessaging::{
        CreateWindowExW, DispatchMessageW, GetMessageW, GetSystemMetrics, LoadCursorW,
        RegisterClassW, ShowWindow, TranslateMessage, CS_HREDRAW, CS_VREDRAW, IDC_ARROW, MSG,
        SM_CXSCREEN, SM_CYSCREEN, SW_SHOW, WNDCLASSW, WS_EX_NOREDIRECTIONBITMAP,
        WS_EX_TOOLWINDOW, WS_EX_TOPMOST, WS_POPUP,
    };

    unsafe {
        let hmodule = GetModuleHandleW(None).map_err(|_| ())?;
        let instance: HINSTANCE = std::mem::transmute(hmodule);

        let class_name: Vec<u16> = "CapscrHotbarGlass\0".encode_utf16().collect();

        let wc = WNDCLASSW {
            style: CS_HREDRAW | CS_VREDRAW,
            lpfnWndProc: Some(hotbar_wnd_proc),
            hInstance: instance,
            lpszClassName: PCWSTR(class_name.as_ptr()),
            hCursor: LoadCursorW(None, IDC_ARROW).unwrap_or_default(),
            ..zeroed()
        };

        RegisterClassW(&wc);

        let screen_width = GetSystemMetrics(SM_CXSCREEN);
        let screen_height = GetSystemMetrics(SM_CYSCREEN);

        let btn_count = config.buttons.len() as i32;
        let hotbar_width = config.size.padding as i32 * 2
            + btn_count * config.size.button_width as i32
            + (btn_count - 1).max(0) * config.size.spacing as i32;
        let hotbar_height = config.size.padding as i32 * 2 + config.size.button_height as i32;

        let (x, y) = match config.position {
            HotbarPosition::Top => ((screen_width - hotbar_width) / 2, 10),
            HotbarPosition::Bottom => {
                ((screen_width - hotbar_width) / 2, screen_height - hotbar_height - 50)
            }
            HotbarPosition::Left => (10, (screen_height - hotbar_height) / 2),
            HotbarPosition::Right => {
                (screen_width - hotbar_width - 10, (screen_height - hotbar_height) / 2)
            }
            HotbarPosition::TopLeft => (10, 10),
            HotbarPosition::TopRight => (screen_width - hotbar_width - 10, 10),
            HotbarPosition::BottomLeft => (10, screen_height - hotbar_height - 50),
            HotbarPosition::BottomRight => {
                (screen_width - hotbar_width - 10, screen_height - hotbar_height - 50)
            }
            HotbarPosition::Custom { x, y } => (x, y),
        };

        let hwnd = CreateWindowExW(
            WS_EX_TOOLWINDOW | WS_EX_TOPMOST | WS_EX_NOREDIRECTIONBITMAP,
            PCWSTR(class_name.as_ptr()),
            PCWSTR::null(),
            WS_POPUP,
            x,
            y,
            hotbar_width,
            hotbar_height,
            None,
            None,
            instance,
            None,
        )
        .map_err(|_| ())?;

        if config.glass.enabled {
            enable_acrylic_blur(hwnd, &config.glass);
        }

        let margins = MARGINS {
            cxLeftWidth: -1,
            cxRightWidth: -1,
            cyTopHeight: -1,
            cyBottomHeight: -1,
        };
        let _ = DwmExtendFrameIntoClientArea(hwnd, &margins);

        let config_box = Box::new(config);
        let config_ptr = Box::into_raw(config_box);
        windows::Win32::UI::WindowsAndMessaging::SetWindowLongPtrW(
            hwnd,
            windows::Win32::UI::WindowsAndMessaging::GWLP_USERDATA,
            config_ptr as isize,
        );

        let _ = ShowWindow(hwnd, SW_SHOW);

        let mut msg: MSG = zeroed();
        while running.load(Ordering::SeqCst) {
            if GetMessageW(&mut msg, None, 0, 0).as_bool() {
                let _ = TranslateMessage(&msg);
                DispatchMessageW(&msg);
            } else {
                break;
            }
        }

        let _ = Box::from_raw(config_ptr);
    }

    Ok(())
}

#[cfg(windows)]
unsafe fn enable_acrylic_blur(hwnd: windows::Win32::Foundation::HWND, glass: &GlassConfig) {
    use std::ffi::c_void;
    use windows::core::PCSTR;
    use windows::Win32::System::LibraryLoader::{GetProcAddress, LoadLibraryA};

    #[repr(C)]
    struct AccentPolicy {
        accent_state: u32,
        accent_flags: u32,
        gradient_color: u32,
        animation_id: u32,
    }

    #[repr(C)]
    struct WindowCompositionAttribData {
        attrib: u32,
        pv_data: *mut c_void,
        cb_data: usize,
    }

    const ACCENT_ENABLE_ACRYLICBLURBEHIND: u32 = 4;
    const ACCENT_ENABLE_HOSTBACKDROP: u32 = 5;
    const WCA_ACCENT_POLICY: u32 = 19;

    let user32 = LoadLibraryA(PCSTR(b"user32.dll\0".as_ptr()));
    if user32.is_err() {
        return;
    }
    let user32 = user32.unwrap();

    let set_window_composition_attribute =
        GetProcAddress(user32, PCSTR(b"SetWindowCompositionAttribute\0".as_ptr()));

    if set_window_composition_attribute.is_none() {
        return;
    }

    let set_window_composition_attribute: unsafe extern "system" fn(
        windows::Win32::Foundation::HWND,
        *mut WindowCompositionAttribData,
    ) -> i32 = std::mem::transmute(set_window_composition_attribute.unwrap());

    let gradient_color = (glass.tint_color[3] as u32) << 24
        | (glass.tint_color[2] as u32) << 16
        | (glass.tint_color[1] as u32) << 8
        | (glass.tint_color[0] as u32);

    let mut policy = AccentPolicy {
        accent_state: ACCENT_ENABLE_ACRYLICBLURBEHIND,
        accent_flags: 2,
        gradient_color,
        animation_id: 0,
    };

    let mut data = WindowCompositionAttribData {
        attrib: WCA_ACCENT_POLICY,
        pv_data: &mut policy as *mut _ as *mut c_void,
        cb_data: std::mem::size_of::<AccentPolicy>(),
    };

    let result = set_window_composition_attribute(hwnd, &mut data);

    if result == 0 {
        policy.accent_state = ACCENT_ENABLE_HOSTBACKDROP;
        data.pv_data = &mut policy as *mut _ as *mut c_void;
        set_window_composition_attribute(hwnd, &mut data);
    }
}

#[cfg(windows)]
unsafe extern "system" fn hotbar_wnd_proc(
    hwnd: windows::Win32::Foundation::HWND,
    msg: u32,
    wparam: windows::Win32::Foundation::WPARAM,
    lparam: windows::Win32::Foundation::LPARAM,
) -> windows::Win32::Foundation::LRESULT {
    use windows::Win32::Foundation::{LRESULT, RECT};
    use windows::Win32::Graphics::Gdi::{
        BeginPaint, CreatePen, CreateSolidBrush, DeleteObject, EndPaint, FillRect, RoundRect,
        SelectObject, SetBkMode, SetTextColor, TextOutW, PAINTSTRUCT, PS_SOLID, TRANSPARENT,
    };
    use windows::Win32::UI::WindowsAndMessaging::{
        DefWindowProcW, GetClientRect, GetWindowLongPtrW, PostQuitMessage, GWLP_USERDATA,
        WM_DESTROY, WM_LBUTTONDOWN, WM_NCHITTEST, WM_PAINT, HTCAPTION,
    };

    match msg {
        WM_NCHITTEST => LRESULT(HTCAPTION as isize),
        WM_PAINT => {
            let config_ptr = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *const HotbarConfig;
            if config_ptr.is_null() {
                return DefWindowProcW(hwnd, msg, wparam, lparam);
            }
            let config = &*config_ptr;

            let mut ps: PAINTSTRUCT = std::mem::zeroed();
            let hdc = BeginPaint(hwnd, &mut ps);

            let mut rect: RECT = std::mem::zeroed();
            let _ = GetClientRect(hwnd, &mut rect);

            if !config.glass.enabled {
                let bg_color = rgb_to_colorref(config.theme.background);
                let bg_brush = CreateSolidBrush(bg_color);
                FillRect(hdc, &rect, bg_brush);
                let _ = DeleteObject(bg_brush);
            }

            let btn_brush = CreateSolidBrush(rgb_to_colorref(config.theme.button_background));
            let border_pen = CreatePen(PS_SOLID, 1, rgb_to_colorref(config.theme.border));

            SetBkMode(hdc, TRANSPARENT);
            SetTextColor(hdc, rgb_to_colorref(config.theme.text));

            let mut x = config.size.padding as i32;
            let y = config.size.padding as i32;

            for button in &config.buttons {
                let old_brush = SelectObject(hdc, btn_brush);
                let old_pen = SelectObject(hdc, border_pen);

                let radius = config.theme.border_radius as i32;
                let _ = RoundRect(
                    hdc,
                    x,
                    y,
                    x + config.size.button_width as i32,
                    y + config.size.button_height as i32,
                    radius,
                    radius,
                );

                SelectObject(hdc, old_brush);
                SelectObject(hdc, old_pen);

                let label: Vec<u16> = button.label.encode_utf16().collect();
                let text_x = x + (config.size.button_width as i32 - 8) / 2;
                let text_y = y + (config.size.button_height as i32 - 16) / 2;
                let _ = TextOutW(hdc, text_x, text_y, &label);

                x += config.size.button_width as i32 + config.size.spacing as i32;
            }

            let _ = DeleteObject(btn_brush);
            let _ = DeleteObject(border_pen);

            let _ = EndPaint(hwnd, &ps);
            LRESULT(0)
        }
        WM_LBUTTONDOWN => {
            let config_ptr = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *const HotbarConfig;
            if config_ptr.is_null() {
                return DefWindowProcW(hwnd, msg, wparam, lparam);
            }
            let config = &*config_ptr;

            let click_x = (lparam.0 & 0xFFFF) as i32;
            let click_y = ((lparam.0 >> 16) & 0xFFFF) as i32;

            let mut btn_x = config.size.padding as i32;
            let btn_y = config.size.padding as i32;

            for button in &config.buttons {
                let btn_right = btn_x + config.size.button_width as i32;
                let btn_bottom = btn_y + config.size.button_height as i32;

                if click_x >= btn_x && click_x < btn_right && click_y >= btn_y && click_y < btn_bottom
                {
                    if let Some(ref hotkey) = button.hotkey {
                        simulate_hotkey(hotkey);
                    }
                    break;
                }

                btn_x += config.size.button_width as i32 + config.size.spacing as i32;
            }

            LRESULT(0)
        }
        WM_DESTROY => {
            PostQuitMessage(0);
            LRESULT(0)
        }
        _ => DefWindowProcW(hwnd, msg, wparam, lparam),
    }
}

#[cfg(windows)]
fn rgb_to_colorref(rgba: [u8; 4]) -> windows::Win32::Foundation::COLORREF {
    windows::Win32::Foundation::COLORREF(
        rgba[0] as u32 | ((rgba[1] as u32) << 8) | ((rgba[2] as u32) << 16),
    )
}

#[cfg(windows)]
fn simulate_hotkey(hotkey: &str) {
    use windows::Win32::UI::Input::KeyboardAndMouse::{
        SendInput, INPUT, INPUT_0, INPUT_KEYBOARD, KEYBDINPUT, KEYEVENTF_KEYUP, VIRTUAL_KEY,
        VK_CONTROL, VK_SHIFT,
    };

    let parts: Vec<&str> = hotkey.split('+').map(|s| s.trim()).collect();
    let mut inputs: Vec<INPUT> = Vec::new();

    let mut modifiers: Vec<VIRTUAL_KEY> = Vec::new();
    let mut main_key: Option<VIRTUAL_KEY> = None;

    for part in &parts {
        match part.to_uppercase().as_str() {
            "CTRL" | "CONTROL" => modifiers.push(VK_CONTROL),
            "SHIFT" => modifiers.push(VK_SHIFT),
            "ALT" => modifiers.push(VIRTUAL_KEY(0x12)),
            key if key.len() == 1 => {
                let c = key.chars().next().unwrap();
                if c.is_ascii_alphabetic() {
                    main_key = Some(VIRTUAL_KEY(c.to_ascii_uppercase() as u16));
                }
            }
            _ => {}
        }
    }

    for vk in &modifiers {
        inputs.push(INPUT {
            r#type: INPUT_KEYBOARD,
            Anonymous: INPUT_0 {
                ki: KEYBDINPUT {
                    wVk: *vk,
                    wScan: 0,
                    dwFlags: Default::default(),
                    time: 0,
                    dwExtraInfo: 0,
                },
            },
        });
    }

    if let Some(vk) = main_key {
        inputs.push(INPUT {
            r#type: INPUT_KEYBOARD,
            Anonymous: INPUT_0 {
                ki: KEYBDINPUT {
                    wVk: vk,
                    wScan: 0,
                    dwFlags: Default::default(),
                    time: 0,
                    dwExtraInfo: 0,
                },
            },
        });

        inputs.push(INPUT {
            r#type: INPUT_KEYBOARD,
            Anonymous: INPUT_0 {
                ki: KEYBDINPUT {
                    wVk: vk,
                    wScan: 0,
                    dwFlags: KEYEVENTF_KEYUP,
                    time: 0,
                    dwExtraInfo: 0,
                },
            },
        });
    }

    for vk in modifiers.iter().rev() {
        inputs.push(INPUT {
            r#type: INPUT_KEYBOARD,
            Anonymous: INPUT_0 {
                ki: KEYBDINPUT {
                    wVk: *vk,
                    wScan: 0,
                    dwFlags: KEYEVENTF_KEYUP,
                    time: 0,
                    dwExtraInfo: 0,
                },
            },
        });
    }

    if !inputs.is_empty() {
        unsafe {
            SendInput(&inputs, std::mem::size_of::<INPUT>() as i32);
        }
    }
}

impl Default for HotbarPlugin {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(feature = "runtime")]
impl Plugin for HotbarPlugin {
    fn name(&self) -> &str {
        "Hotbar"
    }

    fn version(&self) -> &str {
        "0.1.0"
    }

    fn description(&self) -> &str {
        "Glass-effect floating hotbar for quick capture actions"
    }

    fn on_event(&mut self, _event: &PluginEvent) -> PluginResponse {
        PluginResponse::Continue
    }

    fn on_load(&mut self) {
        if !self.config_path.exists() {
            self.save_config();
        }
        self.start_hotbar();
    }

    fn on_unload(&mut self) {
        self.stop_hotbar();
        self.save_config();
    }
}

impl Drop for HotbarPlugin {
    fn drop(&mut self) {
        self.stop_hotbar();
    }
}
