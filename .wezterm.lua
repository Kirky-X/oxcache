local wezterm = require 'wezterm'
local config = wezterm.config_builder()

-- 颜色方案
config.color_scheme = 'Google Dark (Gogh)'

-- 字体设置
config.font_size = 11
config.font = wezterm.font_with_fallback {
  'JetBrains Mono',
  'Noto Color Emoji',
}
config.line_height = 1.2
config.cell_width = 1.0

-- 窗口设置
config.initial_cols = 120
config.initial_rows = 32
config.window_background_opacity = 0.93
config.window_decorations = 'INTEGRATED_BUTTONS|RESIZE'
config.window_padding = {
  left = 5,
  right = 5,
  top = 5,
  bottom = 0,
}
config.window_close_confirmation = 'NeverPrompt'

-- 渲染设置
config.front_end = 'OpenGL'
config.max_fps = 60
config.animation_fps = 60

-- 光标设置
config.default_cursor_style = 'BlinkingBlock'
config.cursor_blink_rate = 500
config.cursor_blink_ease_in = 'Constant'
config.cursor_blink_ease_out = 'Constant'
config.hide_mouse_cursor_when_typing = true

-- 滚动设置
config.scrollback_lines = 10000
config.enable_scroll_bar = true

-- 默认启动 WSL (openEuler-24.03)，打开用户主目录
config.default_prog = { 'wsl.exe', '-d', 'openEuler-24.03', '--cd', '~' }

-- 启动菜单
config.launch_menu = {
  {
    label = 'WSL openEuler-24.03',
    args = { 'wsl.exe', '-d', 'openEuler-24.03', '--cd', '~' },
  },
  {
    label = 'WSL openEuler (指定目录)',
    args = { 'wsl.exe', '-d', 'openEuler-24.03' },
    cwd = '\\\\wsl$\\openEuler-24.03\\home',
  },
  {
    label = 'PowerShell',
    args = { 'powershell.exe', '-NoLogo' },
  },
  {
    label = 'Command Prompt',
    args = { 'cmd.exe' },
  },
  {
    label = 'Git Bash',
    args = { 'C:\\Program Files\\Git\\bin\\bash.exe', '-i', '-l' },
  },
}

-- 标签栏设置
config.enable_tab_bar = true
config.hide_tab_bar_if_only_one_tab = false
config.use_fancy_tab_bar = true
config.tab_bar_at_bottom = false
config.tab_max_width = 32
config.switch_to_last_active_tab_when_closing_tab = true

-- 窗格设置
config.inactive_pane_hsb = {
  saturation = 0.9,
  brightness = 0.8,
}
config.pane_focus_follows_mouse = false

-- 行为设置
config.automatically_reload_config = true
config.audible_bell = 'Disabled'
config.check_for_updates = false
config.term = 'xterm-256color'

-- Leader 键设置
config.leader = {
  key = 'Space',
  mods = 'CTRL',
  timeout_milliseconds = 1000,
}

-- 快捷键
config.keys = {
  -- Ctrl+Shift+T 新建标签页 (WSL)
  {
    key = 'T',
    mods = 'CTRL|SHIFT',
    action = wezterm.action.SpawnCommandInNewTab {
      args = { 'wsl.exe', '-d', 'openEuler-24.03', '--cd', '~' },
    },
  },
  -- Ctrl+Shift+N 新建窗口 (WSL)
  {
    key = 'N',
    mods = 'CTRL|SHIFT',
    action = wezterm.action.SpawnCommandInNewWindow {
      args = { 'wsl.exe', '-d', 'openEuler-24.03', '--cd', '~' },
    },
  },
  -- Ctrl+Shift+L 打开启动菜单
  {
    key = 'L',
    mods = 'CTRL|SHIFT',
    action = wezterm.action.ShowLauncher,
  },
  -- Ctrl+Shift+P 打开命令面板
  {
    key = 'P',
    mods = 'CTRL|SHIFT',
    action = wezterm.action.ActivateCommandPalette,
  },
  -- Alt+Left/Right 切换标签页
  {
    key = 'LeftArrow',
    mods = 'ALT',
    action = wezterm.action.ActivateTabRelative(-1),
  },
  {
    key = 'RightArrow',
    mods = 'ALT',
    action = wezterm.action.ActivateTabRelative(1),
  },
  -- Ctrl+Shift+W 关闭当前标签页
  {
    key = 'W',
    mods = 'CTRL|SHIFT',
    action = wezterm.action.CloseCurrentTab { confirm = true },
  },
  -- Leader 键模式：窗格分割
  {
    key = '|',
    mods = 'LEADER|SHIFT',
    action = wezterm.action.SplitHorizontal { domain = 'CurrentPaneDomain' },
  },
  {
    key = '-',
    mods = 'LEADER',
    action = wezterm.action.SplitVertical { domain = 'CurrentPaneDomain' },
  },
  {
    key = 'x',
    mods = 'LEADER',
    action = wezterm.action.CloseCurrentPane { confirm = true },
  },
  -- Leader 键模式：窗格导航
  {
    key = 'h',
    mods = 'LEADER',
    action = wezterm.action.ActivatePaneDirection 'Left',
  },
  {
    key = 'j',
    mods = 'LEADER',
    action = wezterm.action.ActivatePaneDirection 'Down',
  },
  {
    key = 'k',
    mods = 'LEADER',
    action = wezterm.action.ActivatePaneDirection 'Up',
  },
  {
    key = 'l',
    mods = 'LEADER',
    action = wezterm.action.ActivatePaneDirection 'Right',
  },
  -- Leader 键模式：窗格大小调整
  {
    key = 'H',
    mods = 'LEADER|SHIFT',
    action = wezterm.action.AdjustPaneSize { 'Left', 5 },
  },
  {
    key = 'J',
    mods = 'LEADER|SHIFT',
    action = wezterm.action.AdjustPaneSize { 'Down', 5 },
  },
  {
    key = 'K',
    mods = 'LEADER|SHIFT',
    action = wezterm.action.AdjustPaneSize { 'Up', 5 },
  },
  {
    key = 'L',
    mods = 'LEADER|SHIFT',
    action = wezterm.action.AdjustPaneSize { 'Right', 5 },
  },
  -- Leader 键模式：窗格旋转
  {
    key = 'r',
    mods = 'LEADER',
    action = wezterm.action.RotatePanes 'Clockwise',
  },
  -- Leader 键模式：显示窗格编号
  {
    key = 'q',
    mods = 'LEADER',
    action = wezterm.action.PaneSelect,
  },
  -- Leader 键模式：重载配置
  {
    key = 'R',
    mods = 'LEADER|SHIFT',
    action = wezterm.action.ReloadConfiguration,
  },
}

-- 鼠标设置
config.mouse_bindings = {
  -- 右键粘贴
  {
    event = { Down = { streak = 1, button = 'Right' } },
    mods = 'NONE',
    action = wezterm.action.PasteFrom 'Clipboard',
  },
  -- Ctrl+点击打开链接
  {
    event = { Up = { streak = 1, button = 'Left' } },
    mods = 'CTRL',
    action = wezterm.action.OpenLinkAtMouseCursor,
  },
}

return config
