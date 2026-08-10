cmake_minimum_required(VERSION 3.16)
project(playtune_gui CXX)

# Enable automatic compilation of Qt files
set(CMAKE_AUTOMOC ON)
set(CMAKE_AUTOUIC ON)
set(CMAKE_AUTORCC ON)

set(CMAKE_CXX_STANDARD 17)
set(CMAKE_CXX_STANDARD_REQUIRED ON)

# Find Qt6 components
find_package(Qt6 COMPONENTS Widgets OpenGL OpenGLWidgets REQUIRED)

# Define the source files for our modular UI library
set(SOURCES
    src/apptheme.cpp
    src/apptheme.h
    src/gui_bridge.cpp
    src/gui_bridge.h
    src/mainwindow.cpp
    src/mainwindow.h
    src/mainwindow_bridge.cpp
    src/mainwindow_events.cpp
    src/mainwindow_actions.cpp
    src/tooltip_controller.cpp
    src/tooltip_controller.h
    src/equalizerwindow.cpp
    src/equalizerpresets.cpp
    src/equalizerbands.cpp
    src/equalizerwindow.h
    src/custom_widgets.cpp
    src/custom_widgets.h
    src/coverloader.cpp
    src/coverloader.h
    src/mediagridview.cpp
    src/mediagridview.h
    src/sidebar.cpp
    src/sidebar.h
    src/nowplayingcard.cpp
    src/nowplayingcard.h
    src/playing_equalizer_icon.cpp
    src/playing_equalizer_icon.h
    src/songstable.cpp
    src/songstable.h
    src/songstable_rendering.cpp
    src/songstable_actions.cpp
    src/queuewidget.cpp
    src/queuewidget.h
    src/settingspage.cpp
    src/settingspage_audio.cpp
    src/settingspage_library.cpp
    src/settingspage.h
    src/foldersview.cpp
    src/foldersview.h
    src/albumsview.cpp
    src/albumsview.h
    src/artistsview.cpp
    src/artistsview.h
    src/playlistcreatedialog.cpp
    src/playlistcreatedialog.h
    src/sleeptimerdialog.cpp
    src/sleeptimerdialog.h
    src/tageditordialog.cpp
    src/tageditordialog.h
    src/loudnessscannerdialog.cpp
    src/loudnessscannerdialog.h
    src/lrcparser.cpp
    src/lrcparser.h
    src/karaokedialog.cpp
    src/karaokedialog.h
    resources.qrc
)

# Create the static library
add_library(playtune_gui STATIC ${SOURCES})

# B-04: use PUBLIC (not PRIVATE) for Qt6 so consumers (the Rust build.rs
# linker and the demo executable) automatically link Qt6::Widgets/Gui/Core
# without having to re-discover them. For a STATIC library, PRIVATE link
# libraries are NOT propagated to consumers.
target_link_libraries(playtune_gui PUBLIC
    Qt6::Widgets
    Qt6::Core
    Qt6::Gui
    Qt6::OpenGL
    Qt6::OpenGLWidgets
)

# Set include directory permissions so compiling is straightforward
target_include_directories(playtune_gui PUBLIC src)

# G-50: the demo previously re-compiled ALL library sources into the
# executable instead of linking against playtune_gui. This doubled
# compilation time and could cause ODR violations. We now link the demo
# against the static library.
add_executable(playtune_demo src/main_demo.cpp)
target_link_libraries(playtune_demo PRIVATE playtune_gui Qt6::Widgets Qt6::Core Qt6::Gui)
target_include_directories(playtune_demo PRIVATE src)

# Specify install rule so cargo can locate it in the prefix folder
install(TARGETS playtune_gui
    ARCHIVE DESTINATION lib
    LIBRARY DESTINATION lib
)
