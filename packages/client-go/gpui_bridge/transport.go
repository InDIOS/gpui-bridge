package gpui_bridge

import (
	"bufio"
	"encoding/json"
	"fmt"
	"io"
	"os"
	"os/exec"
	"strings"
	"sync"
	"time"
)

// HostEvents informa de lo que sucede mientras el host corre.
type HostEvents struct {
	// OnMessage recibe cada mensaje host → cliente ya parseado (clave `type`).
	OnMessage func(message map[string]any)
	// OnCorruptLine recibe una línea de stdout que no es un mensaje de protocolo.
	OnCorruptLine func(line string)
	// OnExit se invoca cuando el host termina.
	OnExit func(exit HostExit)
}

// HostExit describe cómo terminó el host.
type HostExit struct {
	// Código de salida del proceso, si se pudo leer.
	Code *int
	// Nombre de la señal, si terminó por señal.
	Signal string
	// Últimas líneas que el host escribió a stderr, para informes de fallo.
	StderrTail []string
	// Error de arranque, si el proceso ni siquiera pudo iniciarse.
	SpawnError error
}

// HostError es un mensaje `error` del host: la operación fue rechazada y el host sigue vivo.
type HostError struct {
	Code    string
	Message string
	Op      string
}

// HostExitedError se devuelve cuando se opera contra un host ya terminado.
type HostExitedError struct {
	Message string
	Exit    HostExit
}

func (e *HostExitedError) Error() string {
	return e.Message
}

const stderrTailLines = 20

// Transport posee un proceso de host arrancado y sus flujos JSON Lines stdin/stdout.
type Transport struct {
	proc   *os.Process
	state  *os.ProcessState
	stdin  io.WriteCloser
	stdout *bufio.Reader
	stderr *bufio.Reader

	stderrMu   sync.Mutex
	stderrTail []string

	waitErr error
	done    chan struct{}
}

// Spawn arranca el host y devuelve un Transport listo para Send/ReadLine.
func Spawn(path string, args ...string) (*Transport, error) {
	cmd := exec.Command(path, args...)
	stdinPipe, err := cmd.StdinPipe()
	if err != nil {
		return nil, fmt.Errorf("host %s: no se pudo crear stdin: %w", path, err)
	}
	stdoutPipe, err := cmd.StdoutPipe()
	if err != nil {
		return nil, fmt.Errorf("host %s: no se pudo crear stdout: %w", path, err)
	}
	stderrPipe, err := cmd.StderrPipe()
	if err != nil {
		return nil, fmt.Errorf("host %s: no se pudo crear stderr: %w", path, err)
	}
	if err := cmd.Start(); err != nil {
		return nil, fmt.Errorf("host no encontrado en %s: %w", path, err)
	}

	t := &Transport{
		proc:   cmd.Process,
		stdin:  stdinPipe,
		stdout: bufio.NewReader(stdoutPipe),
		stderr: bufio.NewReader(stderrPipe),
		done:   make(chan struct{}),
	}
	go t.drainStderr()
	go t.wait()
	return t, nil
}

func (t *Transport) drainStderr() {
	scanner := bufio.NewScanner(t.stderr)
	for scanner.Scan() {
		line := scanner.Text()
		t.stderrMu.Lock()
		t.stderrTail = append(t.stderrTail, line)
		if len(t.stderrTail) > stderrTailLines {
			t.stderrTail = t.stderrTail[1:]
		}
		t.stderrMu.Unlock()
	}
}

func (t *Transport) wait() {
	defer close(t.done)
	t.state, t.waitErr = t.proc.Wait()
}

// Send envía un mensaje cliente como una línea JSON terminada en \n.
func (t *Transport) Send(message ClientMessage) error {
	line, err := json.Marshal(message)
	if err != nil {
		return err
	}
	_, err = t.stdin.Write(append(line, '\n'))
	return err
}

// SendRaw envía una línea ya serializada (útil para reproducir transcripciones byte a byte).
func (t *Transport) SendRaw(line string) error {
	_, err := t.stdin.Write(append([]byte(line), '\n'))
	return err
}

// ReadLine espera la siguiente línea de salida del host.
func (t *Transport) ReadLine() (string, error) {
	line, err := t.stdout.ReadString('\n')
	if err != nil {
		return "", err
	}
	return strings.TrimRight(line, "\r\n"), nil
}

// ReadLineTimeout espera como máximo d la siguiente línea de salida del host.
func (t *Transport) ReadLineTimeout(d time.Duration) (string, error) {
	type res struct {
		line string
		err  error
	}
	ch := make(chan res, 1)
	go func() {
		line, err := t.stdout.ReadString('\n')
		ch <- res{strings.TrimRight(line, "\r\n"), err}
	}()
	select {
	case r := <-ch:
		return r.line, r.err
	case <-time.After(d):
		return "", fmt.Errorf("timeout esperando salida del host")
	}
}

// Close cierra stdin; el host sale al recibir EOF.
func (t *Transport) Close() error {
	return t.stdin.Close()
}

// Wait espera a que el host termine y devuelve su HostExit.
func (t *Transport) Wait() HostExit {
	<-t.done
	t.stderrMu.Lock()
	tail := append([]string{}, t.stderrTail...)
	t.stderrMu.Unlock()

	exit := HostExit{StderrTail: tail}
	if t.state != nil && t.state.Exited() {
		code := t.state.ExitCode()
		exit.Code = &code
	}
	if t.waitErr != nil {
		exit.SpawnError = t.waitErr
	}
	return exit
}

// Mensajes host → cliente (clave `type`), parseados a map para comparar con las
// transcripciones y despachar en App.
func parseHostMessage(line string) (map[string]any, error) {
	var msg map[string]any
	if err := json.Unmarshal([]byte(line), &msg); err != nil {
		return nil, err
	}
	return msg, nil
}
