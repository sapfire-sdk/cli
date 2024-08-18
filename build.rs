#[cfg(windows)]
extern crate winres;

fn main() {
	#[cfg(windows)]
	{
		let mut res = winres::WindowsResource::new();
		res.set_icon("sapfire.ico");
		res.compile().unwrap();
	}
}
