# /dev/null - write discards, read returns empty
echo "discard" > /dev/null
echo "visible"
echo "also discard" >> /dev/null
echo "still visible"
