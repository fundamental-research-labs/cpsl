# Hardware info synthetic files

# /proc/cpuinfo should have processor and model name entries
cat /proc/cpuinfo | head -2

# /proc/meminfo should have MemTotal
cat /proc/meminfo | head -1 | cut -d: -f1

# /etc/os-release first line should be NAME
cat /etc/os-release | head -1

# uname -m should show real architecture
uname -m
