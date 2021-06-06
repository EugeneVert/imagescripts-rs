use std::path::Path;

pub fn ims_init(input: &mut Vec<String>, output_dir: &str, nproc: Option<usize>) {
    if input.get(0).unwrap() == "./*" {
        input_get_from_cwd(input);
    }
    if !Path::new(&output_dir).exists() {
        std::fs::create_dir_all(&output_dir)
            .unwrap_or_else(|_| panic!("Error creating dir {}", &output_dir));
    }
    if let Some(n) = nproc {
        rayon::ThreadPoolBuilder::new()
            .num_threads(n)
            .build_global()
            .unwrap()
    }
}

/// Gather image-files from cwd, remove 1'st element from input Vec
///
/// # Examples
/// ```
/// struct Opt {
///     #[structopt(required = false, default_value = "./*", display_order = 0)]
///     input: Vec<String>,
/// }
/// let opt = Opt::from_iter(args);
/// let mut images = opt.input.to_owned();
/// if images.get(0).unwrap() == "./*" {
///     images_get_from_cwd(&mut images);
/// }
/// ```
pub fn input_get_from_cwd(input: &mut Vec<String>) {
    let image_formats = ["png", "jpg", "webp"];
    input.append(
        &mut std::path::Path::new(".")
            .read_dir()
            .unwrap()
            .map(|dir| dir.unwrap().path().into_os_string().into_string().unwrap())
            .collect::<Vec<String>>(),
    );
    input.remove(0);
    input.retain(|i| image_formats.iter().any(|&format| i.ends_with(format)));
}
