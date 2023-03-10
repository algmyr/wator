use pixels::{Error, Pixels, SurfaceTexture};
use rand::seq::{IteratorRandom, SliceRandom};
use rand::Rng;
use winit::dpi::LogicalSize;
use winit::event::{Event, VirtualKeyCode};
use winit::event_loop::{ControlFlow, EventLoop};
use winit::window::WindowBuilder;
use winit_input_helper::WinitInputHelper;

const SCALE: usize = 2; // To scale size and starting number proportionally.
const WIDTH: usize = 320 * SCALE;
const HEIGHT: usize = 240 * SCALE;

type TimeType = u8;

#[derive(Copy, Clone, PartialEq, Eq, Hash)]
struct Point {
  x: isize,
  y: isize,
}

fn nudge_into_range(x: isize, m: isize) -> isize {
  if x < 0 {
    x + m
  } else if x >= m {
    x - m
  } else {
    x
  }
}

impl Point {
  fn from_ix(ix: usize) -> Self {
    Self { x: (ix % WIDTH) as isize, y: (ix / WIDTH) as isize }
  }

  fn offset(&self, dx: isize, dy: isize) -> Point {
    let x = nudge_into_range(self.x + dx, WIDTH as isize);
    let y = nudge_into_range(self.y + dy, HEIGHT as isize);
    Point { x, y }
  }
}

#[derive(Copy, Clone, PartialEq, Eq)]
struct Shark {
  pos: Point,
  repro_time: TimeType,
  starve: TimeType,
}

impl Shark {
  fn new(pos: Point) -> Self { Self { pos, repro_time: 0, starve: 0 } }
}

#[derive(Copy, Clone, PartialEq, Eq)]
struct Fish {
  pos: Point,
  repro_time: TimeType,
}

impl Fish {
  fn new(pos: Point) -> Self { Self { pos, repro_time: 0 } }
}

#[repr(u8)]
#[derive(Copy, Clone, PartialEq, Eq)]
enum Content {
  Empty = 0,

  Fish = 1,
  NewFish = 1 | 4,

  Shark = 2,
  NewShark = 2 | 4,
  FedShark = 2 | 8,
}

impl Content {
  fn is_empty(&self) -> bool {
    *self as u8 == 0
  }
  fn is_fish(&self) -> bool {
    *self as u8 & 1 != 0
  }
  #[allow(unused)]
  fn is_shark(&self) -> bool {
    *self as u8 & 2 != 0
  }
}

struct Board {
  data: Vec<Content>,
}

impl Board {
  fn new() -> Self { Self { data: vec![Content::Empty; WIDTH * HEIGHT] } }

  fn get(&self, p: Point) -> Content {
    let ix = p.y as usize * WIDTH + p.x as usize;
    self.data[ix]
  }

  fn get_mut(&mut self, p: Point) -> &mut Content {
    let ix = p.y as usize * WIDTH + p.x as usize;
    &mut self.data[ix]
  }
}

fn clear_by_cond<T: Copy>(
  vec: &mut Vec<T>,
  should_remove: impl Fn(&T) -> bool,
) -> Vec<T> {
  let mut i = 0;
  let mut removed = vec![];
  while i < vec.len() {
    if should_remove(&vec[i]) {
      removed.push(vec.swap_remove(i));
    } else {
      i += 1;
    }
  }
  removed
}

struct World {
  occupied: Board,
  sharks: Vec<Shark>,
  fishes: Vec<Fish>,
  fish_repro_time: TimeType,
  shark_repro_time: TimeType,
  shark_starves: TimeType,
}

impl World {
  fn new(
    n_sharks: usize,
    n_fish: usize,
    fish_repro_time: TimeType,
    shark_repro_time: TimeType,
    shark_starves: TimeType,
  ) -> Self {
    let mut occupied = Board::new();

    let rng = &mut rand::thread_rng();
    let mut indices = (0..WIDTH * HEIGHT)
      .choose_multiple(rng, n_fish + n_sharks)
      .into_iter();

    let mut fishes = vec![];
    for ix in indices.by_ref().take(n_fish) {
      let fish =
        Fish { pos: Point::from_ix(ix), repro_time: rng.gen_range(1..=fish_repro_time) };
      *occupied.get_mut(fish.pos) = Content::Fish;
      fishes.push(fish);
    }

    let mut sharks = vec![];
    for ix in indices {
      let shark = Shark {
        pos: Point::from_ix(ix),
        repro_time: rng.gen_range(1..=shark_repro_time as u8),
        starve: 0,
      };
      *occupied.get_mut(shark.pos) = Content::Shark;
      sharks.push(shark);
    }

    World { occupied, sharks, fishes, fish_repro_time, shark_repro_time, shark_starves }
  }

  fn update(&mut self) {
    let rng = &mut rand::thread_rng();
    let mut directions = [(-1, 0), (1, 0), (0, -1), (0, 1)];

    let mut new_fishes = vec![];
    for fish in &mut self.fishes {
      directions.shuffle(rng);
      let start = fish.pos;

      // Move.
      for (dx, dy) in directions {
        if self.occupied.get(fish.pos.offset(dx, dy)).is_empty() {
          *self.occupied.get_mut(fish.pos) = Content::Empty;
          fish.pos = fish.pos.offset(dx, dy);
          *self.occupied.get_mut(fish.pos) = Content::Fish;
          break;
        }
      }

      // Breed if moved.
      if start != fish.pos {
        fish.repro_time += 1;
        if fish.repro_time >= self.fish_repro_time {
          fish.repro_time = 0;
          new_fishes.push(Fish::new(start));
          *self.occupied.get_mut(start) = Content::NewFish;
        }
      }
    }
    self.fishes.extend(new_fishes);

    let mut fishes_to_remove = std::collections::HashSet::new();
    let mut new_sharks = vec![];
    for shark in &mut self.sharks {
      directions.shuffle(rng);
      let start = shark.pos;

      // Eat.
      for (dx, dy) in directions {
        if self.occupied.get(shark.pos.offset(dx, dy)).is_fish() {
          fishes_to_remove.insert(shark.pos.offset(dx, dy));
          shark.starve = 0;
          *self.occupied.get_mut(shark.pos) = Content::Empty;
          shark.pos = shark.pos.offset(dx, dy);
          *self.occupied.get_mut(shark.pos) = Content::FedShark;
          break;
        }
      }

      // Move if not already moved.
      if start == shark.pos {
        for (dx, dy) in directions {
          if self.occupied.get(shark.pos.offset(dx, dy)).is_empty() {
            *self.occupied.get_mut(shark.pos) = Content::Empty;
            shark.pos = shark.pos.offset(dx, dy);
            *self.occupied.get_mut(shark.pos) = Content::Shark;
            break;
          }
        }
      }

      // Breed if moved.
      if start != shark.pos {
        shark.repro_time += 1;
        if shark.repro_time == self.shark_repro_time {
          shark.repro_time = 0;
          new_sharks.push(Shark::new(start));
          *self.occupied.get_mut(start) = Content::NewShark;
        }
      }

      shark.starve += 1;
    }
    self.sharks.extend(new_sharks);

    // Clear eaten fish.
    clear_by_cond(&mut self.fishes, |&fish| {
      fishes_to_remove.contains(&fish.pos)
    });

    // Kill starved sharks.
    for rem in clear_by_cond(&mut self.sharks, |&shark| {
      shark.starve >= self.shark_starves
    }) {
      *self.occupied.get_mut(rem.pos) = Content::Empty;
    }
  }
}

struct Sim {
  world: World,
}

impl Sim {
  fn new() -> Self {
    Self { world: World::new(1000 * SCALE * SCALE, 3000 * SCALE * SCALE, 60, 35, 30) }
  }

  fn update(&mut self) { self.world.update(); }

  fn draw(&self, frame: &mut [u8]) {
    for (i, pixel) in frame.chunks_exact_mut(4).enumerate() {
      let p = Point::from_ix(i);

      let rgba = match self.world.occupied.get(p) {
        Content::Empty    => [0x00, 0x00, 0x00, 0xff],

        Content::Fish     => [0x00, 0x99, 0x00, 0xff],
        Content::NewFish  => [0x00, 0xff, 0x00, 0xff],

        Content::Shark    => [0xff, 0x00, 0x00, 0xff],
        Content::NewShark => [0xff, 0xff, 0xff, 0xff],
        Content::FedShark => [0xff, 0xff, 0x00, 0xff],
      };

      pixel.copy_from_slice(&rgba);
    }
  }
}

fn main() -> Result<(), Error> {
  let event_loop = EventLoop::new();
  let mut input = WinitInputHelper::new();
  let window = {
    let size = LogicalSize::new(WIDTH as f64, HEIGHT as f64);
    WindowBuilder::new()
      .with_title("Hello Pixels")
      .with_inner_size(size)
      .with_min_inner_size(size)
      .build(&event_loop)
      .unwrap()
  };

  let mut pixels = {
    let window_size = window.inner_size();
    let surface_texture =
      SurfaceTexture::new(window_size.width, window_size.height, &window);
    Pixels::new(WIDTH as u32, HEIGHT as u32, surface_texture)?
  };
  let mut sim = Sim::new();

  let mut i = 0;
  event_loop.run(move |event, _, control_flow| {
    // Draw the current frame
    if let Event::RedrawRequested(_) = event {
      if let Err(err) = pixels.render() {
        eprintln!("pixels.render() failed: {err}");
        *control_flow = ControlFlow::Exit;
        return;
      }
      sim.update();
      sim.draw(pixels.get_frame_mut());
      window.request_redraw();

      i += 1;
      //if i == 1000 {
      //  *control_flow = ControlFlow::Exit;
      //  return;
      //}
    }

    // Handle input events
    if input.update(&event) {
      // Close events
      if input.key_pressed(VirtualKeyCode::Escape) || input.close_requested() {
        *control_flow = ControlFlow::Exit;
        return;
      }

      // Resize the window
      if let Some(size) = input.window_resized() {
        if let Err(err) = pixels.resize_surface(size.width, size.height) {
          eprintln!("pixels.resize_surface() failed: {err}");
          *control_flow = ControlFlow::Exit;
          return;
        }
      }

      // Update internal state and request a redraw
      sim.update();
      window.request_redraw();
    }
  });
}
