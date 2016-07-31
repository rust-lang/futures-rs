# Where did all these certs come from?

That's a good question! The exacty way in which custom certificates are
specified to each SSL/TLS backend is pretty hairy, so we end up having the same
cert in a couple of formats.

In the end though all of these come from the schannel-rs project (hence the
schannel-ca.crt). This is because they're carefully crafted to work well on
Windows.

If you'd like to regenerate them, check out the script in that repository.
