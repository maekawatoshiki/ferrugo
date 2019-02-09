class GameOfLife {
  static final int grid_height   = 20;
  static final int grid_width    = 20;
  static boolean   grid[][]      = null;
  static boolean   copy_grid[][] = null;

  static void fill_with_random() {
    for (int x = 0; x < grid_width; x++) {
      for (int y = 0; y < grid_height; y++) {
        grid[x][y] = Math.random() < 0.5 ? true : false;
      }
    }
  }

  static void draw_grid() {
    System.out.println("-----");
    for (int y = 1; y < grid_height; y++) {
      for (int x = 1; x < grid_width; x++) {
        System.out.print(grid[x][y] ? "#" : " ");
      }
      System.out.println("");
    }
  }

  static void update_grid() {
    for (int y = 1; y < grid_height - 1; y++) {
      for (int x = 1; x < grid_width - 1; x++) {
        int total_cells = 0;
    
        total_cells += grid[(x - 1)][(y - 1)] ? 1 : 0;
        total_cells += grid[(x - 1)][(y   ) ] ? 1 : 0;
        total_cells += grid[(x - 1)][(y + 1)] ? 1 : 0;
        total_cells += grid[(x    )][(y - 1)] ? 1 : 0;
        total_cells += grid[(x    )][(y + 1)] ? 1 : 0;
        total_cells += grid[(x + 1)][(y - 1)] ? 1 : 0;
        total_cells += grid[(x + 1)][(y    )] ? 1 : 0;
        total_cells += grid[(x + 1)][(y + 1)] ? 1 : 0;
    
        if (grid[x][y]) {
          if (total_cells == 0 || total_cells == 1) {
            copy_grid[x][y] = false; 
          } else if (total_cells == 2 || total_cells == 3) {
            copy_grid[x][y] = true; 
          } else if (4 <= total_cells && total_cells <= 8) {
            copy_grid[x][y] = false; 
          } else {
            copy_grid[x][y] = false;
          }
        } else { 
          if (total_cells == 3) {
            copy_grid[x][y] = true; 
          } else {
            copy_grid[x][y] = false; 
          }
        }
      }
    }
    
    for (int x = 0; x < grid_width; x++) {
      for (int y = 0; y < grid_height; y++) {
        grid[x][y] = copy_grid[x][y];
      }
    }
  }

  public static void main(String args[]) {
    grid      = new boolean[grid_width][grid_height];
    copy_grid = new boolean[grid_width][grid_height];

    fill_with_random();

    for (int i = 0; i < 10; i++) {
      update_grid();
      draw_grid();
    }
  }
}
