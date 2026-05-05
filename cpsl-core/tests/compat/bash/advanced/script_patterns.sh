# Common script patterns
# String building
GREETING="hello"
GREETING="$GREETING world"
echo "$GREETING"

# Conditional file processing (with fs)
mkdir -p /tmp/script_test
echo "test content" > /tmp/script_test/data.txt

if [ -f /tmp/script_test/data.txt ]; then
    echo "file exists"
    cat /tmp/script_test/data.txt
else
    echo "no file"
fi

# For loop processing
for name in alice bob charlie; do
    echo "processing: $name"
done

# Clean up
rm /tmp/script_test/data.txt
rm -r /tmp/script_test
echo "done"
