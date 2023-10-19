use clap::Parser;
use lazy_static::lazy_static;
use std::{
    collections::{HashMap, HashSet},
    path::PathBuf,
    sync::{
        atomic::{AtomicUsize, Ordering},
        Arc, Mutex,
    },
};

lazy_static! {
    pub static ref VIDEO_EXTENSIONS: Vec<String> = vec![
        "mp4".into(),
        "avi".into(),
        "flv".into(),
        "heic".into(),
        "mkv".into(),
        "mov".into(),
        "mpg".into(),
        "mpeg".into(),
        "m4v".into(),
        "webm".into(),
        "wmv".into(),
        "3gp".into()
    ];
}

/// Configuration for the video server.
#[derive(Parser, Debug, Clone)]
pub struct VideoPlayerConfig {
    #[clap(short, long, default_value = "assets")]
    pub assets_root: String,

    #[clap(short, long, default_value = "9092")]
    pub port: u16,

    #[clap(short, long, default_value = "0.0.0.0")]
    pub host: String,
}

impl Default for VideoPlayerConfig {
    fn default() -> Self {
        Self {
            assets_root: "assets".to_string(),
            port: 9092,
            host: "0.0.0.0".to_string(),
        }
    }
}

/// Shared state for the video server, including video indexing.
#[derive(Default)]
pub struct VideoPlayerState {
    pub videos: HashMap<String, String>,
    video_extensions: HashSet<String>,
    next_index: AtomicUsize,
    root: Option<String>,
}

pub type SharedState = Arc<Mutex<VideoPlayerState>>;

impl VideoPlayerState {
    /// Create a new video index state.
    /// This function configures the supported video file extensions.
    pub fn new() -> Self {
        Self {
            video_extensions: HashSet::from_iter(VIDEO_EXTENSIONS.iter().map(|s| s.to_string())),
            ..Default::default()
        }
    }

    /// Increment the index for video files.
    fn advance_index(&mut self) {
        self.next_index
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    }

    /// Check if a file path is a supported video file.
    pub fn is_video_file<P: AsRef<std::path::Path>>(&self, path: P) -> bool {
        if let Some(extension) = path.as_ref().extension() {
            if self.video_extensions.contains(extension.to_str().unwrap()) {
                return true;
            }
        }
        false
    }

    /// Load videos from a specified directory path.
    pub fn load_videos<P: AsRef<std::path::Path>>(&mut self, root: P) -> std::io::Result<()> {
        self.visit_dirs(root)
    }

    /// Load a video from a file path.
    pub fn load_video(&mut self, path: PathBuf) {
        let stored_file_name = path.to_str().unwrap().to_string();
        let extension = path.extension().unwrap();
        // make server path {id}.{ext}
        // if the first loaded video is an mp4 file,
        // the server path would be "0.mp4"
        // if the next is mov,
        // the server path would be "1.mov"
        let server_path = format!(
            "{}.{}",
            self.next_index.load(Ordering::SeqCst),
            extension.to_str().unwrap()
        );
        println!("Loading video: {} as {}", stored_file_name, server_path);
        // increase index for next video
        self.advance_index();
        // mapping used by axum router
        self.videos.insert(server_path, stored_file_name);
    }

    /// Recursively visit all directories and load videos from them.
    pub fn visit_dirs<P: AsRef<std::path::Path>>(&mut self, root: P) -> std::io::Result<()> {
        if root.as_ref().is_dir() {
            // if given path is valid directory
            if let Ok(dir) = std::fs::read_dir(root.as_ref()) {
                for entry in dir {
                    let entry = entry?;
                    let path = entry.path();
                    // if entry within directory is another directory
                    if path.is_dir() {
                        // call self recursively
                        self.visit_dirs(path)?;
                    }
                    // otherwise, if is video file
                    else if self.is_video_file(path.as_path()) {
                        // load video
                        self.load_video(path);
                    }
                    // ignore all other file types
                }
            }
        }
        Ok(())
    }

    /// Build a new video index state from a configuration.
    pub fn build(config: &VideoPlayerConfig) -> Self {
        let mut state = Self::new();
        state.root = Some(config.assets_root.clone());
        state.load_videos(state.root.clone().unwrap()).unwrap();
        state
    }

    /// Reload the video index state, resetting the index and clearing the video list.
    pub fn reload(&mut self) {
        self.next_index = AtomicUsize::new(0);
        self.videos.clear();
        self.load_videos(self.root.clone().unwrap()).unwrap();
    }
}
