set -ex

# Create a new certificate
openssl req -nodes -new -x509  -keyout server.key -out server.crt \
  -subj "/C=US/ST=Denial/L=Springfield/O=Dis/CN=localhost"

# Export to der format
openssl x509 -outform der -in server.crt -out server.der

# Export to pkcs12 format
openssl pkcs12 -export -nodes -out server.p12 -inkey server.key -in server.crt \
  -password pass:foobar
