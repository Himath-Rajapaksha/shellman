import os, pty, sys, time, re, shutil, select
src = open(os.path.join(os.path.dirname(os.path.abspath(__file__)), '..', 'src', 'shell.rs')).read()
BASHRC = src.split('const BASHRC: &str = r#"')[1].split('"#;')[0]
T = sys.argv[1]
shutil.rmtree(T, ignore_errors=True); os.makedirs(T + '/home')
open(T + '/rc', 'w').write(BASHRC)
extra = sys.argv[2] if len(sys.argv) > 2 else ''
open(T + '/home/.bashrc', 'w').write('. /etc/profile.d/vte-2.91.sh\nHISTFILE=%s/hist\n%s\n' % (T, extra))
evil = T + '/d1 a\'b;$(touch ' + T + '/PWNED)`touch ' + T + '/PWNED2`\x1b\x05\r\nend'
os.makedirs(evil); os.makedirs(T + '/d2'); os.makedirs(T + '/d3')
open(T + '/sel', 'wb').write(b'/x/a b\0/x/l\'e\nte\0')
env = dict(os.environ, HOME=T + '/home', TERM='xterm-256color', VTE_VERSION='8400',
           SHELLMAN_SEL=T + '/sel', SHELLMAN_CD=T + '/cd', SHELLMAN_STATE=T + '/state', PS1='$ ')
pid, fd = pty.fork()
if pid == 0:
    os.execvpe('bash', ['bash', '--rcfile', T + '/rc', '-i'], env)
out = b''
def pump(t=0.3):
    global out
    end = time.time() + t
    while time.time() < end:
        r, _, _ = select.select([fd], [], [], 0.05)
        if r:
            try: out += os.read(fd, 65536)
            except OSError: return
def state():
    try: return open(T + '/state').read()
    except: return None
def wait_prompt(t=5):
    end = time.time() + t
    while time.time() < end:
        pump(0.1)
        if state() == 'prompt': return True
    return False
def cwd(): return os.readlink('/proc/%d/cwd' % pid)
def cd(path):
    open(T + '/cd', 'wb').write(path.encode() + b'\0')
    os.write(fd, b'\x1b[24;8~')
def run(cmd): os.write(fd, cmd.encode() + b'\r'); pump(0.4); wait_prompt()
ok = lambda name, cond: print(('OK   ' if cond else 'FAIL ') + name)

ok('prompt reached, state "prompt"', wait_prompt())
cd(evil); pump(0.5); wait_prompt()
ok('cd on empty line: hostile folder reached', cwd() == evil)
ok('no injected command executed', not os.path.exists(T + '/PWNED') and not os.path.exists(T + '/PWNED2'))
os.write(fd, b'echo typed > ' + T.encode() + b'/f1'); pump(0.2)
cd(T + '/d2'); pump(0.5)
ok('cd while typing: folder reached', cwd() == T + '/d2')
ok('line being typed not executed by the cd', not os.path.exists(T + '/f1'))
os.write(fd, b'\r'); pump(0.4); wait_prompt()
ok('line being typed kept, then executed', open(T + '/f1').read() == 'typed\n' if os.path.exists(T + '/f1') else False)
run('printf "%s|" "${SEL[@]}" > ' + T + '/sel_out; echo ${#SEL[@]} >> ' + T + '/sel_out')
ok('SEL read (2 names, one with a newline)', open(T + '/sel_out').read() == "/x/a b|/x/l'e\nte|2\n")
run('false'); run('echo $? > ' + T + '/st')
ok('$? preserved', open(T + '/st').read() == '1\n')
run('echo one last'); run('echo "$_" > ' + T + '/us')
ok('$_ preserved', open(T + '/us').read() == 'last\n')
os.write(fd, b'sleep 1\r'); pump(0.3)
ok('state "busy" while a command runs', state() == 'busy')
wait_prompt()
ok('state "prompt" after the command', state() == 'prompt')
run('set -o vi')
cd(T + '/d3'); pump(0.5); wait_prompt()
ok('vi mode (insert): cd done', cwd() == T + '/d3')
os.write(fd, b'\x1b'); pump(0.6)
cd(T + '/d2'); pump(0.5); wait_prompt()
ok('vi mode (command): cd done', cwd() == T + '/d2')
run('set -o emacs')
run('history > ' + T + '/hist_out')
h = open(T + '/hist_out').read()
ok('history has no cd and no key sequence', 'cd' not in h and '9999' not in h)
if extra:
    ok('existing DEBUG trap still called', os.path.exists(T + '/dbg') and os.path.getsize(T + '/dbg') > 0)
os.write(fd, b'exit\r'); pump(0.5)
