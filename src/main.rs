use pixels::{Error, Pixels, SurfaceTexture};
use rand::Rng;
use rand::seq::{IteratorRandom, SliceRandom};
use winit::dpi::LogicalSize;
use winit::event::Event;
use winit::event_loop::{ControlFlow, EventLoop};
use winit::window::WindowBuilder;

const SCALE: usize = 2; // To scale size and starting number proportionally.
const WIDTH: usize = 320*SCALE;
const HEIGHT: usize = 240*SCALE;

type TimeType = u8;

#[derive(Copy, Clone, PartialEq, Eq, Hash)]
struct Point {
  x: isize,
  y: isize,
}

impl Point {
  fn from_ix(ix: usize) -> Self {
    Self { x: (ix%WIDTH) as isize, y: (ix/WIDTH) as isize }
  }

  fn offset(&self, dx:isize, dy: isize) -> Point {
    Point { x: (self.x + dx).rem_euclid(WIDTH as isize), y: (self.y + dy).rem_euclid(HEIGHT as isize) }
  }
}

#[derive(Copy, Clone, PartialEq, Eq)]
struct Shark {
  pos: Point,
  repro_time: TimeType,
  starve: TimeType,
}

#[derive(Copy, Clone, PartialEq, Eq)]
struct Fish {
  pos: Point,
  repro_time: TimeType,
}

struct Board {
  data: Vec<u8>,
}

impl Board {
  fn new() -> Self { Self { data: vec![0; WIDTH * HEIGHT] } }

  fn get(&self, p: Point) -> u8 {
    let ix = p.y.rem_euclid(HEIGHT as isize) as usize* WIDTH + p.x.rem_euclid(WIDTH as isize) as usize;
    self.data[ix]
  }

  fn get_mut(&mut self, p: Point) -> &mut u8 {
    let ix = p.y.rem_euclid(HEIGHT as isize) as usize* WIDTH + p.x.rem_euclid(WIDTH as isize) as usize;
    &mut self.data[ix]
  }
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
    let mut indices = (0..WIDTH*HEIGHT).choose_multiple(rng, n_fish + n_sharks).into_iter();

    let mut fishes = vec![];
    for ix in indices.by_ref().take(n_fish) {
      let fish = Fish { pos: Point::from_ix(ix), repro_time: rng.gen_range(1..=fish_repro_time) };
      *occupied.get_mut(fish.pos) = 1;
      fishes.push(fish);
    }

    let mut sharks = vec![];
    for ix in indices {
      let shark = Shark { pos: Point::from_ix(ix), repro_time: rng.gen_range(1..=shark_repro_time as u8), starve: 0 };
      *occupied.get_mut(shark.pos) = 2;
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
        if self.occupied.get(fish.pos.offset(dx, dy)) == 0 {
          *self.occupied.get_mut(fish.pos) = 0;
          fish.pos = fish.pos.offset(dx, dy);
          *self.occupied.get_mut(fish.pos) = 1;
          break;
        }
      }

      // Breed.
      if start != fish.pos {
        fish.repro_time += 1;
        if fish.repro_time >= self.fish_repro_time {
          fish.repro_time = 0;
          new_fishes.push(Fish { pos: start, repro_time: 0 } );
          *self.occupied.get_mut(start) = 1;
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
        if self.occupied.get(shark.pos.offset(dx, dy)) == 1 {
          fishes_to_remove.insert(shark.pos.offset(dx, dy));
          //if let Some(ix) = self.fishes.iter().position(|fish| fish.pos == shark.pos.offset(dx, dy)) {
          //  self.fishes.swap_remove(ix);
          //}
          shark.starve = 0;
          *self.occupied.get_mut(shark.pos) = 0;
          shark.pos = shark.pos.offset(dx, dy);
          *self.occupied.get_mut(shark.pos) = 2;
          break;
        }
      }

      // Move if not already moved.
      if start == shark.pos {
        for (dx, dy) in directions {
          if self.occupied.get(shark.pos.offset(dx, dy)) == 0 {
            *self.occupied.get_mut(shark.pos) = 0;
            shark.pos = shark.pos.offset(dx, dy);
            *self.occupied.get_mut(shark.pos) = 2;
            break;
          }
        }
      }

      // Breed.
      if start != shark.pos {
        shark.repro_time += 1;
        if shark.repro_time == self.shark_repro_time {
          shark.repro_time = 0;
          new_sharks.push(Shark { pos: start, repro_time: 0, starve: 0 } );
          *self.occupied.get_mut(start) = 2;
        }
      }

      shark.starve += 1;
    }
    self.sharks.extend(new_sharks);

    // Clear eaten fish.
    {
      let mut i = 0;
      for j in 0..self.fishes.len() {
        if fishes_to_remove.contains(&self.fishes[j].pos) {
        } else {
          self.fishes[i] = self.fishes[j];
          i += 1;
        }
      }
      self.fishes.drain(i..);
    }

    // Kill starved sharks.
    {
      let mut i = 0;
      for j in 0..self.sharks.len() {
        if self.sharks[j].starve >= self.shark_starves {
          *self.occupied.get_mut(self.sharks[j].pos) = 0;
        } else {
          self.sharks[i] = self.sharks[j];
          i += 1;
        }
      }
      self.sharks.drain(i..);
    }
  }
}

struct Sim {
  world: World,
}

impl Sim {
  fn new() -> Self {
    Self { world: World::new(1000*SCALE*SCALE, 3000*SCALE*SCALE, 60, 35, 30) }
  }

  fn update(&mut self) {
    self.world.update();
  }

  fn draw(&self, frame: &mut [u8]) {
    for (i, pixel) in frame.chunks_exact_mut(4).enumerate() {
      let p = Point::from_ix(i);

      let rgba = match self.world.occupied.get(p) {
        1 => [0x00, 0xff, 0x00, 0xff], // Fish
        2 => [0xff, 0x00, 0x00, 0xff], // Shark
        _ => [0x00, 0x00, 0x00, 0x00],
      };
      
      pixel.copy_from_slice(&rgba);
    }
  }
}

fn main() -> Result<(), Error> {
  let event_loop = EventLoop::new();
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
    }
  });
}
