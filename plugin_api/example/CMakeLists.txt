cmake_minimum_required(VERSION 3.7)
project(websocat_plugin_example)

add_library(websocat_plugin_example SHARED
    websocat_plugin_example.c
)

target_include_directories(websocat_plugin_example PRIVATE ..)
set_target_properties(websocat_plugin_example PROPERTIES PUBLIC_HEADER ../websocat_plugin.h)
