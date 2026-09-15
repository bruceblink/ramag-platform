# Keep Visual Studio projects on the vcvarsall.bat environment.
set(CMAKE_VS_GLOBALS "UseEnv=true" CACHE STRING "Use the active Visual Studio environment" FORCE)

# Propagate the Visual Studio environment setting to CMake ABI probes.
set(CMAKE_TRY_COMPILE_PLATFORM_VARIABLES "CMAKE_VS_GLOBALS" CACHE STRING "CMake variables forwarded to try_compile" FORCE)
