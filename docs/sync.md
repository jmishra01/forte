# Encrypted sync

Encrypted sync keeps a notes folder in step between computers through a
folder that something else already syncs: iCloud Drive, Dropbox, Google
Drive, Syncthing, or a USB stick you carry around. Forte only ever writes
encrypted files there.

## Setting it up

1. **Settings → Encrypted sync → Choose folder…** and pick an empty folder
   inside iCloud Drive (or Dropbox, …).
2. Choose a passphrase and type it twice.
3. On the other computer, open the same notes in Forte (or an empty notes
   folder), choose the same synced folder, and enter the same passphrase.

Forte syncs when you click the cloud button, at startup, and every 5 minutes
if **Sync automatically** is on.

## What happens to changes

For each note and attachment, Forte compares it with how it was at the last
sync:

| Changed here | Changed there | Result |
| --- | --- | --- |
| yes | no | sent |
| no | yes | received (a deletion moves the note to the trash) |
| yes | yes | yours stays; theirs is saved as "Note (Laptop's conflicted copy …)" and shown under **Sync conflicts** |
| deleted | edited | the edit wins |

## What is protected

- **Contents and names are hidden.** Each file is stored as
  `objects/<id>.bin`, where the id is a keyed hash of its path. The folder
  shows how many files you have and roughly how large they are, nothing else.
- **Encryption:** XChaCha20-Poly1305, with a key made from your passphrase by
  Argon2id (64 MiB, 3 passes) and a random salt.
- **Tampering is detected.** Each file is bound to its id; a file that was
  changed, damaged or swapped fails to decrypt and is skipped (Forte tells you
  how many).
- **Your passphrase** is kept in the system keychain, never in the notes or
  sync folders.

Not covered: someone who can write to the sync folder could put back an
*older* encrypted copy of a note they once saw, which you would receive as a
change. Only use a sync folder you control.

**If you forget the passphrase, nobody can open the encrypted copy**,
including you. The notes on each computer are unaffected; set up sync again
with a new, empty folder.
