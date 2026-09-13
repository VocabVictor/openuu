#ifndef RUNNER_WIN32_WINDOW_CLASS_H_
#define RUNNER_WIN32_WINDOW_CLASS_H_

#include <windows.h>

// The number of Win32Window objects that currently exist (defined in
// win32_window_class.cpp, counted by Win32Window).
extern int g_active_window_count;

// Manages the Win32Window's window class registration.
class WindowClassRegistrar {
 public:
  ~WindowClassRegistrar() = default;

  // Returns the singleton registrar instance.
  static WindowClassRegistrar* GetInstance() {
    if (!instance_) {
      instance_ = new WindowClassRegistrar();
    }
    return instance_;
  }

  // Returns the name of the window class, registering the class if it hasn't
  // previously been registered.
  const wchar_t* GetWindowClass();

  // Unregisters the window class. Should only be called if there are no
  // instances of the window.
  void UnregisterWindowClass();

 private:
  WindowClassRegistrar() = default;

  static WindowClassRegistrar* instance_;

  bool class_registered_ = false;
};

const wchar_t* getWindowClassName();

#endif  // RUNNER_WIN32_WINDOW_CLASS_H_
