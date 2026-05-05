# Synthetic directories: /proc, /etc, /tmp

# /proc exists and is listable
if test -d /proc; then echo "proc_is_dir"; fi
ls /proc

# /proc/version contains sandbox identification
cat /proc/version

# /etc exists and is listable
if test -d /etc; then echo "etc_is_dir"; fi
ls /etc

# /etc/hostname
cat /etc/hostname

# /tmp is writable
mkdir -p /tmp
echo "hello" > /tmp/test.txt
cat /tmp/test.txt
echo "world" >> /tmp/test.txt
cat /tmp/test.txt

# Cleanup
rm /tmp/test.txt
if test -e /tmp/test.txt; then echo "WRONG"; else echo "cleaned"; fi
