# Extended file operations
# Create directory and files
mkdir -p /tmp/testdir
echo "file one" > /tmp/testdir/a.txt
echo "file two" > /tmp/testdir/b.txt
echo "file three" > /tmp/testdir/c.txt

# cat single file
cat /tmp/testdir/a.txt

# cat multiple files
cat /tmp/testdir/a.txt /tmp/testdir/b.txt

# ls
ls /tmp/testdir

# cp
cp /tmp/testdir/a.txt /tmp/testdir/d.txt
cat /tmp/testdir/d.txt

# mv
mv /tmp/testdir/d.txt /tmp/testdir/e.txt
cat /tmp/testdir/e.txt

# Append (>>)
echo "appended line" >> /tmp/testdir/a.txt
cat /tmp/testdir/a.txt

# touch
touch /tmp/testdir/empty.txt
ls /tmp/testdir

# pwd
cd /tmp/testdir
pwd

# Clean up
rm /tmp/testdir/a.txt /tmp/testdir/b.txt /tmp/testdir/c.txt /tmp/testdir/e.txt /tmp/testdir/empty.txt
rm -r /tmp/testdir
