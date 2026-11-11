import os, pty, sys, time, shutil, select
src = open(os.path.join(os.path.dirname(os.path.abspath(__file__)), '..', 'src', 'shell.rs')).read()
def const(name): return src.split('const %s: &str = r#"' % name)[1].split('"#;')[0]
SHELL = sys.argv[1]; T = sys.argv[2]
shutil.rmtree(T, ignore_errors=True); os.makedirs(T + '/home')
evil = T + '/d1 a\'b;$(touch ' + T + '/PWNED)`touch ' + T + '/PWNED2`\x1b\x05\r\nend'
os.makedirs(evil); os.makedirs(T + '/d2'); os.makedirs(T + '/d3')
open(T + '/sel', 'wb').write(b'/x/a b\0/x/l\'e\nte\0')
env = dict(os.environ, HOME=T + '/home', TERM='xterm-256color',
           SHELLMAN_SEL=T + '/sel', SHELLMAN_CD=T + '/cd', SHELLMAN_STATE=T + '/state')
env.pop('ZDOTDIR', None)
if SHELL == 'zsh':
    os.makedirs(T + '/zd'); open(T + '/zd/.zshenv', 'w').write(const('ZSHENV'))
    open(T + '/home/.zshrc', 'w').write("PROMPT='%%~$ '\nHISTFILE=%s/hist\nSAVEHIST=100\nHISTSIZE=100\n" % T)
    env['ZDOTDIR'] = T + '/zd'
    argv = ['zsh', '-i']
else:
    open(T + '/init.fish', 'w').write(const('FISH_INIT'))
    os.makedirs(T + '/home/.config/fish')
    open(T + '/home/.config/fish/config.fish', 'w').write('function fish_greeting; end\n')
    env['XDG_CONFIG_HOME'] = T + '/home/.config'; env['XDG_DATA_HOME'] = T + '/home/.local/share'
    argv = ['fish', '--init-command', "source '%s/init.fish'" % T]
pid, fd = pty.fork()
if pid == 0:
    os.chdir('/tmp'); os.execvpe(argv[0], argv, env)
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
def wait_prompt(t=6):
    end = time.time() + t
    while time.time() < end:
        pump(0.1)
        if state() == 'prompt': return True
    return False
def cwd(): return os.readlink('/proc/%d/cwd' % pid)
def cd(path):
    open(T + '/cd', 'wb').write(path.encode() + b'\0'); os.write(fd, b'\x1b[24;8~')
def run(cmd): os.write(fd, cmd.encode() + b'\r'); pump(0.5); wait_prompt()
ok = lambda name, cond: print(('OK   ' if cond else 'FAIL ') + name)
f = lambda p: open(p).read() if os.path.exists(p) else None

ok('prompt reached, state "prompt"', wait_prompt())
cd(evil); pump(0.6); wait_prompt()
ok('cd: hostile folder reached', cwd() == evil)
ok('no injected command executed', not os.path.exists(T + '/PWNED') and not os.path.exists(T + '/PWNED2'))
os.write(fd, ('echo typed > %s/f1' % T).encode()); pump(0.3)
cd(T + '/d2'); pump(0.6)
ok('cd while typing: folder reached', cwd() == T + '/d2')
ok('line being typed not executed by the cd', not os.path.exists(T + '/f1'))
os.write(fd, b'\r'); pump(0.5); wait_prompt()
ok('line being typed kept, then executed', f(T + '/f1') == 'typed\n')
if SHELL == 'zsh':
    run('printf "%%s|" "${SEL[@]}" > %s/sel_out; echo ${#SEL} >> %s/sel_out' % (T, T))
else:
    run('printf "%%s|" $SEL > %s/sel_out; count $SEL >> %s/sel_out' % (T, T))
ok('SEL read (2 names, one with a newline)', f(T + '/sel_out') == "/x/a b|/x/l'e\nte|2\n")
run('false'); run(('echo $? > %s/st' if SHELL == 'zsh' else 'echo $status > %s/st') % T)
ok('exit status preserved', f(T + '/st') == '1\n')
os.write(fd, b'sleep 1\r'); pump(0.4)
ok('state "busy" while a command runs', state() == 'busy')
wait_prompt()
ok('state "prompt" after the command', state() == 'prompt')
run('bindkey -v' if SHELL == 'zsh' else 'fish_vi_key_bindings')
cd(T + '/d3'); pump(0.6); wait_prompt()
ok('vi mode (insert): cd done', cwd() == T + '/d3')
os.write(fd, b'\x1b'); pump(0.6)
cd(T + '/d2'); pump(0.6); wait_prompt()
ok('vi mode (command): cd done', cwd() == T + '/d2')
os.write(fd, b'i'); pump(0.2)
run('bindkey -e' if SHELL == 'zsh' else 'fish_default_key_bindings')
run(('fc -ln 1 > %s/hist_out' if SHELL == 'zsh' else 'history > %s/hist_out') % T)
h = f(T + '/hist_out') or ''
ok('history has no cd and no key sequence', ' cd ' not in h and 'cd /' not in h and '9999' not in h)
os.write(fd, b'exit\r'); pump(0.5)
