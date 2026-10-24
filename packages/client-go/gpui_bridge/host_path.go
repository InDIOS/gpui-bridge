package gpui_bridge

import (
	"os"
	"path/filepath"
	"runtime"
)

// defaultHostPath resuelve el binario del host con el mismo orden que los otros clientes
// (PROTOCOL.md §1, implícito en el comportamiento de createApp / App):
//
//  1. $GPUI_BRIDGE_HOST, si está definido.
//  2. bin/ junto a este paquete (donde lo deja stage-host.mjs o el paquete publicado).
//  3. Una build de Cargo caminando hacia arriba desde este paquete
//     (target/debug o target/release).
//
// Ninguna de estas opciones requiere conocer cuántos niveles separan al cliente de la raíz
// del repositorio, así que sobrevive a que el cliente se mueva dentro de packages/.
func defaultHostPath() string {
	if env := os.Getenv("GPUI_BRIDGE_HOST"); env != "" {
		return env
	}
	exe := "gpui-bridge-host"
	if runtime.GOOS == "windows" {
		exe += ".exe"
	}

	// Directorio de este paquete (gpui_bridge/), independientemente del cwd.
	_, file, _, _ := runtime.Caller(0)
	pkgDir := filepath.Dir(file)

	if bin := filepath.Join(pkgDir, "bin", exe); exists(bin) {
		return bin
	}

	dir := pkgDir
	for i := 0; i < 8; i++ {
		dir = filepath.Dir(dir)
		for _, profile := range []string{"debug", "release"} {
			cand := filepath.Join(dir, "target", profile, exe)
			if exists(cand) {
				return cand
			}
		}
		if dir == filepath.Dir(dir) {
			break // llegó a la raíz del sistema de archivos
		}
	}
	return filepath.Join(pkgDir, "..", "..", "..", "target", "debug", exe) // fallback
}

func exists(path string) bool {
	_, err := os.Stat(path)
	return err == nil
}
