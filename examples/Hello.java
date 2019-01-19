class Point {
  int x, y;

  Point() {
    x = 0;
    y = 0;
  }

  public void show() {
    System.out.println("x = " + x + ", y = " + y);
  }
}

class Person {
  String name;

  Person(String new_name) {
    name = new_name;
  }

  public void show() {
    System.out.println("I'm " + name);
  }
}

class Teacher extends Person {
  String subject;

  Teacher(String new_name, String new_subject) {
    super(new_name);
    subject = new_subject;
  }

  public void show() {
    System.out.println("I'm " + name + ", teaching " + subject);
  }
}

class Hello {
  public static int fibo(int n) {
    if (n <= 2) return 1;
    else return fibo(n - 1) + fibo(n - 2);
  }

  public static boolean is_prime(int n) {
    if (n == 2) return true;
    if (n % 2 == 0 || n <= 1) return false;
    for (int k = 3; k * k <= n; k += 2) 
      if (n % k == 0) 
        return false;
    return true;
  }

  public static double arctan(double x) {
    double ret = 0.0;
    double sig = x;
    double sqx = x * x;
    for (int i = 0; sig != 0.0; i++) {
      ret += sig / (double) (i + i + 1);
      sig = -sig * sqx;
    }
    return ret;
  }

  public static double calc_pi() {
    double pi = 16.0 * arctan(1.0 / 5.0) - 4.0 * arctan(1.0 / 239.0);
    return pi;
  }

  public static double mandelbrot(double c_x, double c_y, int n) {
    double x_n = 0, y_n = 0, x_n_1 = 0, y_n_1 = 0;
    for (int i = 0; i < n; i++) {
      x_n_1 = x_n * x_n - (y_n * y_n) + c_x;
      y_n_1 = 2.0 * x_n * y_n + c_y;
      double t = x_n_1 * x_n_1 + y_n_1 * y_n_1;
      if (t > 4.0) {
        return t;
      } else {
        x_n = x_n_1;
        y_n = y_n_1;
      }
    }
    return 0.0;
  }

  public static void main(String[] args) {
    String hello = "hello";
    System.out.println(hello.length());
    System.out.println("hello" == hello);
    
    int count = 0;
    for (int i = 0; i < 10000; i++) {
      if (i % 33 == 0) continue;
      for (int k = 0; k < 10000; k++) {
        count += k % 2 == 0 ? i : -1;
      }
    }
    System.out.println("jit test " + count);
    
    System.out.println(123);
    System.out.println(123456789);
    {
      int x = 10, y = 7;
      System.out.println(x + " & " + y + " = " + (x & y));
    }
    System.out.println(calc_pi());
    
    char a = 'a';
    short q = 6553, w = 3;
    q += w;
    
    System.out.println("fibo(36) = " + fibo(36));
    
    Point p = new Point();
    p.x = 2;
    p.y = 3;
    p.show();
    
    int z = 2;
    if (z == 2) {
      z = 3;
    } else {
      z = 5;
    }
    System.out.println(z);
    
    Person person = new Person("carol");
    person.show();
    Teacher eng = new Teacher("carol", "English");
    eng.show();
    
    for (int i = 1; i < 500000; i++) {
      if (is_prime(i)) System.out.println(i);
    }
    
    int arr[] = new int[8];
    for (int i = 0; i < 8; i++) 
      arr[i] = i * 2;
    for (int i = 0; i < 8; i++) 
      System.out.println("arr[" + i + "] = " + arr[i]);
    
    Person people[] = new Person[2];
    people[0] = new Person("alice");
    people[0].show();
    people[1] = new Person("bob");
    people[1].show();
    
    double x_max = 2, x_min = -2, y_max = 1, y_min = -1, dx = 0.03, dy = 0.045;
    for (double y = y_max; y > y_min; y -= dy) {
      for (double x = x_min; x < x_max; x += dx) {
        double t = mandelbrot(x, y, 300);
        System.out.print(t > 8 ? "#" : t > 6 ? "*" : t > 4 ? "." : " ");
      }
      System.out.println("");
    }
  }
}
