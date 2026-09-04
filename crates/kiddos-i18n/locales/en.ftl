# KidDOS voice — English.
# The machine speaks in first person, short sentences, kid register.

boot-hello = Hello! I am KidDOS. Type hi and press Enter.
motd = Type help if you get lost. Type man ls to read about ls.
hi = Hi { $name }! I'm your computer. I only understand words I know.
    Type help to see them. Type man <word> to learn one.
hi-first = Hi! I'm your computer. What's your name?
hi-named = Nice to meet you, { $name }. I'll remember that.

unknown-command = I don't know "{ $cmd }". Try help, or man -k { $cmd }.
did-you-mean = Did you mean { $cmd }?
not-found = I can't find { $path }.
is-dir = { $path } is a folder, not a file.
not-dir = { $path } is not a folder.
permission-denied = { $path } belongs to the machine. You can't change it. (Permission denied)
exists = { $path } is already here.
not-empty = { $path } is not empty. Empty it first, or use rm -r.
rm-forever = rm is forever. There is no trash can. Gone is gone.
rm-dir-hint = { $path } is a folder. To remove a folder and everything in it, use rm -r { $path }.

nowhere-to-exit = There is nowhere to go — this is the whole computer! Type help.
program-stopped = Stopped.
program-too-long = Your program is taking too long. Press Ctrl-C to stop it.
bye = Bye!
shutdown = Shutting down. See you later!
reboot = Rebooting...

help-intro = Here is what I know. Type man <word> to learn one, e.g. man ls.
help-more = Type help <topic> for: files, text, machine, learning, programs.
help-topic-files = Working with files and folders
help-topic-text = Playing with text
help-topic-system = The machine itself
help-topic-learning = Learning
help-topic-programs = Programs and games
help-topic-machine = Machine controls

man-no-page = I have no manual page for { $cmd }. Try help.
man-search-none = Nothing matches "{ $q }".
man-press-key = -- more -- (Space: next page, q: quit)

parent-enter-password = Parent password:
parent-set-password = No parent password yet. Choose one:
parent-repeat-password = Again:
parent-mismatch = They don't match. Try again.
parent-wrong = Wrong password.
parent-locked = Too many tries. Wait { $minutes } minutes.
parent-welcome = Parent mode. Type help for parent commands, exit to go back.
parent-only = Only a parent can do that. Type parent.

speak-denied = I'm not allowed to talk right now.
speak-too-fast = Let me catch my breath. (speaking is rate limited)

lang-set = OK, I'll speak English now.
lang-unknown = I only speak { $langs } for now. More languages come later.
crt-on = CRT effects on.
crt-off = CRT effects off.
font-set = Font: { $font }.

usage = Usage: { $usage }
missing-operand = { $cmd } needs something to work on. Try man { $cmd }.
no-such-user = I don't know a user called { $user }.
history-empty = No history yet. Type something!
cd-home-hint = (no folder given, so I took you home)
not-executable = { $path } is not runnable yet. Make it runnable with: chmod +x { $path }
locked-command = { $cmd } is locked. You earn it by finishing a game: play vi-quest
