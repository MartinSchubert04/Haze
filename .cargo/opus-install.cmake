# libopus_sys (lo usa el bot de Discord) le pasa a CMake "-DCMAKE_INSTALL_LIBDIR=lib" sin
# tipo, y CMake lo vuelve absoluto respecto de la carpeta de build: opus.lib termina en
# out/build/lib y el crate la busca en out/lib. Aca se fija donde corresponde.
if(CMAKE_INSTALL_PREFIX)
  set(CMAKE_INSTALL_LIBDIR "${CMAKE_INSTALL_PREFIX}/lib" CACHE PATH "" FORCE)
endif()
