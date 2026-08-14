# Nyanbin CLI

The Nyanbin CLI creates, reveals, and deletes Nyanbin v1 notes. Encryption and decryption happen locally. The browser-safe shared package uses Web Crypto and has no Node-only imports.

## Install

```sh
npm install --global nyanbin
nyanbin --help
```

Node.js 22 or newer is required. The default server is `http://localhost:8000`; select another self-hosted server with `--server` or `NYANBIN_SERVER`. No public service URL is built in.

## Create

```sh
nyanbin create text 'hello'
nyanbin create text '# hello' --format markdown --expires 1h --max-reads 3
nyanbin create text 'attached' --file photo.png --file notes.txt
nyanbin create file report.pdf photo.png --expires 7d
printf '%s' 'second factor' | nyanbin create text 'secret' --password-stdin
nyanbin create text 'secret' --password 'second factor'
```

Durations are positive seconds, or use `s`, `m`, `h`, or `d`. The server normalizes the requested lifetime during reservation. Its returned absolute lifecycle is authenticated by the encrypted envelope.
`--password` and `--password-stdin` are mutually exclusive. Standard input is read only when `--password-stdin` is present, and the command waits for EOF rather than guessing whether input will arrive. One final LF, CRLF, or CR line ending is removed; every other character, including spaces, tabs, earlier line endings, and Unicode, is part of the password. An empty result is rejected. Do not type `--password-stdin` at an interactive terminal; pipe it from a secret source.

Attachments are opened without following symbolic links, then checked and read through that same file descriptor. Portable attachment names may contain ordinary Unicode but not path separators, controls, bidirectional formatting controls, Windows device names or colon syntax, or leading/trailing dots and spaces.

Successful creation prints two stable lines:

```text
Note: https://self-hosted.example/note/0123456789abcdefghijklmnopqrstuv#base64url-secret
Delete token: base64url-creator-capability
```

The fragment contains the random 32-byte link secret and is never sent to the server. Store the delete token separately; it cannot be recovered and is not part of the shared note URL.

## Reveal and delete

```sh
nyanbin open 'https://self-hosted.example/note/ID#SECRET'
printf '%s' 'second factor' | nyanbin open 'https://self-hosted.example/note/ID#SECRET' --password-stdin --all
nyanbin open 'https://self-hosted.example/note/ID#SECRET' --password 'second factor' --all
nyanbin open 'https://self-hosted.example/note/ID#SECRET' --raw
nyanbin delete 'https://self-hosted.example/note/ID#SECRET' --delete-token 'TOKEN'
```

Reveal is an explicit consuming action. A wrong password or damaged link may consume one read because the server atomically consumes before local decryption. Passive info requests do not consume a read.

By default, decrypted note text is terminal-safe: terminal and bidirectional control characters are printed as visible escapes. `--raw` writes note text verbatim with no added final newline and should be used only when the destination is trusted (for example, a pipe to a file). Attachment files are created with exclusive, no-follow semantics. Existing files and symbolic links are never overwritten; collisions receive a numbered name.

## Shared API

Browser and Node consumers can import `nyanbin/shared`, `nyanbin/shared/api`, or `nyanbin/shared/protocol`. The v1 flow is:

1. `API.reserve({ expiresIn, maxReads? })` returns `{ id, deleteToken, lifecycle }`.
2. `encryptPayload` authenticates protocol, id, and the exact reserved lifecycle.
3. Hash the delete token with `hashDeleteToken`, then `API.commit(id, request)`.
4. `API.info(id)` is passive; `API.reveal(id)` consumes atomically.
5. `decryptPayload` validates and authenticates the closed v1 envelope.
6. `API.delete(id, deleteToken)` sends the capability in a JSON body, never a query string.

The encrypted private manifest contains text format, text, filenames, MIME hints, sizes, and attachment bytes. The server receives only the protocol envelope, authenticated lifecycle policy, envelope length, and delete-token verifier.

Cryptgeon foundation copyright and MIT attribution are preserved in the repository license notices.
