cmake_minimum_required(VERSION 3.10)
project(interactive-feedback-mcp LANGUAGES CXX)

set(CMAKE_CXX_STANDARD 17)
set(CMAKE_CXX_STANDARD_REQUIRED ON)
set(CMAKE_RUNTIME_OUTPUT_DIRECTORY ${CMAKE_BINARY_DIR}/bin)

add_compile_options(/utf-8)

add_executable(feedback_server server.cpp)
add_executable(feedback_gui gui.cpp)

target_compile_definitions(feedback_server PRIVATE
    UNICODE _UNICODE WIN32_LEAN_AND_MEAN NOMINMAX)
target_compile_definitions(feedback_gui PRIVATE
    UNICODE _UNICODE WIN32_LEAN_AND_MEAN NOMINMAX)

set_target_properties(feedback_gui PROPERTIES WIN32_EXECUTABLE TRUE)
target_link_libraries(feedback_gui PRIVATE user32 gdi32 shell32 comctl32)

# 启用 ComCtl32 v6 视觉样式
target_link_options(feedback_gui PRIVATE
    "/MANIFESTDEPENDENCY:type='win32' \
name='Microsoft.Windows.Common-Controls' \
version='6.0.0.0' \
processorArchitecture='*' \
publicKeyToken='6595b64144ccf1df' \
language='*'"
)
