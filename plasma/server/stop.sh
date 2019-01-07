PROCESS_ID=`ps aux | grep server | grep -o -E '[0-9]+' | head -1 | sed -e 's/^0\+//'`
kill -9 $PROCESS_ID
echo server stopped