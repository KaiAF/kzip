# Todo

- [x] Make .kzip extension if it's missing from output
- [x] Recursively read directories
- [x] Make command that lists file names
- [x] Make directory listings save
- [x] Show createdAt and modifiedAt time stamps
- [ ] Generate hashes of files, if kzip reads a file with the same hash as another file, just don't log the buffer and just point to the previous file
- [ ] Attempt to find out how to append to a file, that way this can just load a file, generate the buffer then append then clear the buffer. Better for memory
