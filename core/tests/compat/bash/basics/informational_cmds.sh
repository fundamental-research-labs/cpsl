# Informational commands

# Identity
whoami
hostname
id

# uname variants
uname
uname -a
uname -s
uname -r

# env shows sorted KEY=VALUE pairs (check a known entry)
env | grep "USER="

# export sets a variable
export FOO=bar
env | grep "FOO="

# which
which echo
which nonexistent_cmd

# type
type echo

# ps
ps
