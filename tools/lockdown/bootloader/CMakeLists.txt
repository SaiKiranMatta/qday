# Builds only the second-stage bootloader the chip needs once it's locked. The app is the Rust
# firmware, so the app in this project is an empty stand-in that's never flashed.
cmake_minimum_required(VERSION 3.16)
include($ENV{IDF_PATH}/tools/cmake/project.cmake)
project(qday-bootloader)
