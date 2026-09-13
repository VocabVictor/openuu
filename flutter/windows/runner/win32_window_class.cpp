#include "win32_window_class.h"

#include "win32_window.h"

#include <flutter_windows.h>

#include "resource.h"

#include <string> // for std::wstring

int g_active_window_count = 0;

namespace {

constexpr const wchar_t kWindowClassName[] = L"FLUTTER_RUNNER_WIN32_WINDOW";

// Static variable to hold the custom icon (needs cleanup on exit)
static HICON g_custom_icon_ = nullptr;

// Try to load icon from data\flutter_assets\assets\icon.ico if it exists.
// Returns nullptr if the file doesn't exist or can't be loaded.
HICON LoadCustomIcon() {
  if (g_custom_icon_ != nullptr) {
    return g_custom_icon_;
  }
  wchar_t exe_path[MAX_PATH];
  if (!GetModuleFileNameW(nullptr, exe_path, MAX_PATH)) {
    return nullptr;
  }

  std::wstring icon_path = exe_path;
  size_t last_slash = icon_path.find_last_of(L"\\/");
  if (last_slash == std::wstring::npos) {
    return nullptr;
  }

  icon_path = icon_path.substr(0, last_slash + 1);
  icon_path += L"data\\flutter_assets\\assets\\icon.ico";

  // Check file attributes - reject if missing, directory, or reparse point (symlink/junction)
  DWORD file_attr = GetFileAttributesW(icon_path.c_str());
  if (file_attr == INVALID_FILE_ATTRIBUTES ||
      (file_attr & FILE_ATTRIBUTE_DIRECTORY) ||
      (file_attr & FILE_ATTRIBUTE_REPARSE_POINT)) {
    return nullptr;
  }

  g_custom_icon_ = (HICON)LoadImageW(
      nullptr, icon_path.c_str(), IMAGE_ICON, 0, 0,
      LR_LOADFROMFILE | LR_DEFAULTSIZE);
  return g_custom_icon_;
}

}  // namespace

WindowClassRegistrar* WindowClassRegistrar::instance_ = nullptr;

const wchar_t* WindowClassRegistrar::GetWindowClass() {
  if (!class_registered_) {
    WNDCLASS window_class{};
    window_class.hCursor = LoadCursor(nullptr, IDC_ARROW);
    window_class.lpszClassName = kWindowClassName;
    window_class.style = CS_HREDRAW | CS_VREDRAW;
    window_class.cbClsExtra = 0;
    window_class.cbWndExtra = 0;
    window_class.hInstance = GetModuleHandle(nullptr);
    
    // Try to load icon from data\flutter_assets\assets\icon.ico if it exists
    HICON custom_icon = LoadCustomIcon();
    if (custom_icon != nullptr) {
      window_class.hIcon = custom_icon;
    } else {
      window_class.hIcon =
          LoadIcon(window_class.hInstance, MAKEINTRESOURCE(IDI_APP_ICON));
    }
    
    window_class.hbrBackground = 0;
    window_class.lpszMenuName = nullptr;
    window_class.lpfnWndProc = Win32Window::WndProc;
    RegisterClass(&window_class);
    class_registered_ = true;
  }
  return kWindowClassName;
}

void WindowClassRegistrar::UnregisterWindowClass() {
  UnregisterClass(kWindowClassName, nullptr);
  class_registered_ = false;
  
  // Clean up the custom icon if it was loaded
  if (g_custom_icon_ != nullptr) {
    DestroyIcon(g_custom_icon_);
    g_custom_icon_ = nullptr;
  }
}

const wchar_t* getWindowClassName() {
  return kWindowClassName;
}
