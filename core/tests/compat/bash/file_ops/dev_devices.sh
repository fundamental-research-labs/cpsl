# /dev directory and device files
# ls /dev should list entries
ls /dev | sort | head -3

# /dev/null basics
echo "test" > /dev/null
echo "survived"

# /dev/zero reads as empty
ZERO=$(cat /dev/zero)
echo "zero_len=${#ZERO}"

# test -e checks
if test -e /dev/null; then echo "null_exists"; fi
if test -e /dev/zero; then echo "zero_exists"; fi
if test -e /dev/urandom; then echo "urandom_exists"; fi

# test -d /dev is a directory
if test -d /dev; then echo "dev_is_dir"; fi

# test -f /dev/null is a file
if test -f /dev/null; then echo "null_is_file"; fi

# /dev itself is not a file
if test -f /dev; then echo "WRONG"; else echo "dev_not_file"; fi
