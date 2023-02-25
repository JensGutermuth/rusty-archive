# rusty-archive

Don't let your digital archive rust away! Check for modifications (intentional or not), missing files and track new additions.

## Tutorial

This tool was written to protect the archive of a photographer, so this is what this tutorial will use as an example. There is however nothing about rusty-archive that's specific to images. This should therefore transfer well to any other archive where files are not expected to change or go missing.

### Getting Started

### Install rusty-archive

```
cargo install --git https://github.com/JensGutermuth/rusty-archive.git
rusty-archive --help
```

### Create the initial state of your digital archive

rusty-archive needs a directory to store the state of your digital archive in, so let's create one:

```
mkdir /path/to/state
```

That's.. it. Running your first update will find all the new (to rusty-archive anyway) files and store them in a timestamped file ending with .state.

### Update the state of your digital archive

```shell
rusty-archive update /path/to/state /path/to/your/archive
```

This will walk through your archive and detect any new, modified or missing files. If `--read-all-files` is not given, it will try to avoid reading all files by comparing modification time and size to the previous value. If any files are missing or modified, rusty-archive will create a .missing / .modified file alongside the .state file.

The update command can take additional arguments to - for example - exclude files. Run `rusty-archive update --help` to get a list including descriptions.

### Check if all files found somewhere else are present in the archive:

It's not unusual to have copies of files somewhere other than the archive and these may need to be deleted at some point. Good examples would be a working copy on a local SSD or the SD card the images were shot on. In both cases these have limited space and need to be emptied periodically. To avoid accidentally deleting files that weren't added to the archive yet, run rusty-archive with the `verify` command. It will return an error if any files in the specified directory are not found in the archive state.

The flag `--ignore-missing` tells rusty-archive it's okay to only find a subset of the archive in the specified directory. `--only-presence` just checks if a file with the same contents is in the archive, disregarding paths. By default rusty-archive verifies that an identical copy is found.

```
rusty-archive verify --ignore-missing --only-presence /path/to/state /path/to/sdcard
```

## FAQs

### Can I use a state directory created under a different OS?

Sure! This can be especially useful, if your archive is on a server / NAS running a different OS. You can update the state locally and then for example use `rusty-archive verify --ignore-missing --only-presence` on your desktop to verify you've got all the files off an SD card.

Using rusty-archive from a different OS may result in all files being re-read if paths change. Mixing Windows with anything else will always cause this, as Windows uses `\` instead of `/` to seperate components of paths. rusty-archive will report those files as missing but found elsewhere.

### Can I use this to verify my backups work?

If you can access the contents your backups as a directory, yes! Just run `rusty-archive verify /path/to/state /path/to/your/backup`. Any changed or missing files will be printed to the console. The command will also exit with a non-zero exit code if any changed or missing files are found.

### Doesn't ZFS solve the problem of bitrot way better?

ZFS can detect and — if the array is configured with redudancy — even repair bitrot in most cases. rusty-archive only detects bitrot. Not all changes to files are bitrot, however. Human error (or malice) can cause unintended changes to files or deletions. Flagging those is a big reason rusty-archive exists.

### Why rust?

Why not ;). This also served as a first project to learn some rust. Suggestings about more idiomatic ways to do things are therefore very welcome!

## License

Licensed under either of

 * Apache License, Version 2.0
   ([LICENSE-APACHE](LICENSE-APACHE) or http://www.apache.org/licenses/LICENSE-2.0)
 * MIT license
   ([LICENSE-MIT](LICENSE-MIT) or http://opensource.org/licenses/MIT)

at your option.

## Contribution

Unless you explicitly state otherwise, any contribution intentionally submitted
for inclusion in the work by you, as defined in the Apache-2.0 license, shall be
dual licensed as above, without any additional terms or conditions.