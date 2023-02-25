use bpaf::Bpaf;
use regex::Regex;

fn regex(arg: String) -> Result<Regex, regex::Error> {
    Regex::new(&arg)
}

#[derive(Clone, Debug, Bpaf)]
/// Hash files in a directory tree
#[bpaf(options)]
pub struct Update {}

#[derive(Clone, Debug, Bpaf)]
pub enum Cmd {
    /// Update the archive state
    #[bpaf(command)]
    Update {
        /// Skip comparison of modification times and sizes and read all files
        read_all_files: bool,

        /// directory to store the state in
        #[bpaf(positional::<String>("STATE_DIR"))]
        state_dir: String,

        /// directory to search for files in [default: current directory]
        #[bpaf(positional::<String>("DIRECTORY"))]
        directory: Option<String>,
    },

    /// Verify files based on archive state
    #[bpaf(command)]
    Verify {
        /// Allow files present in the archive state to be missing
        ignore_missing: bool,

        /// Just check files are in the archive, don't verify paths
        only_presence: bool,

        /// directory to store the state in
        #[bpaf(positional::<String>("STATE_DIR"))]
        state_dir: String,

        /// directory to search for files in [default: current directory]
        #[bpaf(positional::<String>("DIRECTORY"))]
        directory: Option<String>,
    },
}

#[derive(Clone, Debug, Bpaf)]
#[bpaf(options, version)]
/// Hash files in a directory tree
pub struct CommandlineOptions {
    /// number of threads to use for reading files [default: 1]
    ///
    /// Increasing this to about 8 increases performance
    /// for reading from SSDs. Increasing this when reading
    /// from HDDs will most likely hurt performance fairly badly.
    #[bpaf(short, long, argument("THREADS"))]
    pub threads: Option<usize>,

    /// Exclude directories matching this regular expression
    ///
    /// Only the name of the directory is checked. Use --exclude-path
    /// to check the full path.
    #[bpaf(argument::<String>("REGEX"), parse(regex), many)]
    pub exclude_directory: Vec<Regex>,

    /// Exclude files matching this regular expression
    ///
    /// Only the name of the file is checked. Use --exclude-path
    /// to check the full path.
    #[bpaf(argument::<String>("REGEX"), parse(regex), many)]
    pub exclude_file: Vec<Regex>,

    /// Exclude files which paths match this regular expression
    #[bpaf(argument::<String>("REGEX"), parse(regex), many)]
    pub exclude_path: Vec<Regex>,

    #[bpaf(external)]
    pub cmd: Cmd,
}
