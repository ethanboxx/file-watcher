#![deny(clippy::pedantic)]
// #![warn(missing_docs)]

use set_error::ChangeError;

use std::{
    path::Path,
    rc::Rc,
    thread,
    time::{Duration, SystemTime},
};

#[derive(Clone)]
pub struct FileListBuilder<T: Clone> {
    files: Vec<WatchedFile<T>>,
    interval: Duration,
    max_retries: Option<u32>,
    open_file_func: Rc<Fn(&str) -> WatchingFuncResult<T>>,
    run_only_once: bool,
}

#[derive(Clone)]
pub struct WatchedFile<T> {
    path: String,
    date_modified: SystemTime,
    functions_on_run: Vec<Rc<Fn(T) -> WatchingFuncResult<T>>>,
    function_on_end: Rc<Fn(T) -> Result<(), String>>,
}

pub enum WatchingFuncResult<T> {
    Success(T),
    Retry(String),
    Fail(String),
}
use WatchingFuncResult::{Fail, Retry, Success};

impl<T: Clone> FileListBuilder<T> {
    pub fn new<F: 'static + Fn(&str) -> WatchingFuncResult<T>>(open_func: F) -> Self {
        Self {
            files: Vec::new(),
            interval: Duration::from_millis(1000),
            max_retries: None,
            open_file_func: Rc::new(open_func),
            run_only_once: false,
        }
    }
    pub fn run_only_once(mut self, q: bool) -> Self {
        self.run_only_once = q;
        self
    }
    pub fn add_file(&mut self, file: WatchedFile<T>) {
        self.files.push(file);
    }
    pub fn with_interval(mut self, inter: Duration) -> Self {
        self.interval = inter;
        self
    }
    pub fn with_max_retries(mut self, re: u32) -> Self {
        self.max_retries = Some(re);
        self
    }
    pub fn launch(mut self) -> Result<(), String> {
        let mut on_first_run = self.files.len() + 1;
        loop {
            for mut file in &mut self.files {
                if on_first_run != 0 {
                    on_first_run -= 1
                }
                if (on_first_run != 0) || (file.date_modified != date_modified(&file.path)?) {
                    file.date_modified = date_modified(&file.path)?;
                    let open_file_func_as_not_mut = self.open_file_func.clone();
                    let mut file_data = keep_doing_until(self.max_retries, self.interval, || {
                        (open_file_func_as_not_mut)(&file.path)
                    })?;
                    for function_to_run in file.functions_on_run.clone() {
                        file_data = keep_doing_until(self.max_retries, self.interval, || {
                            function_to_run(file_data.clone())
                        })?
                    }
                    let mut retries = self.max_retries;
                    loop {
                        match (file.function_on_end)(file_data.clone()) {
                            Ok(_) => break,
                            Err(s) => {
                                retries = retries.map(|x| x - 1);
                                match retries {
                                    Some(n) if n == 0 => {
                                        return Err(String::from("no more retries"))
                                    }
                                    _ => {
                                        println!("{}", s);
                                        thread::sleep(self.interval);
                                        continue;
                                    }
                                }
                            }
                        }
                    }
                    thread::sleep(self.interval);
                }
            }
            if self.run_only_once {
                return Ok(());
            }
        }
    }
}

fn keep_doing_until<F, T>(mut retries: Option<u32>, interval: Duration, f: F) -> Result<T, String>
where
    F: Fn() -> WatchingFuncResult<T>,
{
    Ok(loop {
        match f() {
            Success(t) => break t,
            Fail(s) => return Err(s),
            Retry(s) => {
                retries = retries.map(|x| x - 1);
                match retries {
                    Some(n) if n == 0 => return Err(String::from("no more retries")),
                    _ => {
                        println!("{}", s);
                        thread::sleep(interval);
                        continue;
                    }
                }
            }
        }
    })
}

impl<T> WatchedFile<T> {
    pub fn new<G: 'static + Fn(T) -> Result<(), String>>(
        path: &str,
        end_func: G,
    ) -> Result<Self, String> {
        Ok(Self {
            path: path.to_string(),
            date_modified: date_modified(&path)?,
            functions_on_run: Vec::new(),
            function_on_end: Rc::new(end_func),
        })
    }
    pub fn add_func<F: 'static + Fn(T) -> WatchingFuncResult<T>>(&mut self, func: F) {
        self.functions_on_run.push(Rc::new(func));
    }
}

fn date_modified(path: &str) -> Result<SystemTime, String> {
    Ok(Path::new(path)
        .metadata()
        .set_error(&format!("failed to open file {} metadata", path))?
        .modified()
        .set_error(&format!("failed to find files date modified {}", path))?)
}
