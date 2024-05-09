# Todo

- [x] Make .kzip extension if it's missing from output
- [x] Recursively read directories
- [x] Make command that lists file names
- [x] Make directory listings save
  - I want to store the names without having the trail i.e. `../../folder/*`
- [x] Show createdAt and modifiedAt time stamps
- [x] Generate hashes of files, if kzip reads a file with the same hash as another file, just don't log the buffer and just point to the previous file
  - Clean the code up a bit
- [ ] Attempt to find out how to append to a file, that way this can just load a file, generate the buffer then append then clear the buffer. Better for memory
- [ ] Clean up code
  - Seperate stuff into a utils file. Create a struct for file info, etc.
