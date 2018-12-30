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

  public static void main(String[] args) {
    System.out.println(123);
    System.out.println(123456789);
    System.out.println(calc_pi());
    
    char a = 'a';
    short x = 6553, y = 3;
    x += y;
    
    fibo(1); 
    for (int i = 1; i <= 20; i++)
      System.out.println(fibo(i));
    
    Point p = new Point();
    p.x = 2;
    p.y = 3;
    p.show();

    int i = 2;
    if (i == 2) {
      i = 3;
    } else {
      i = 5;
    }
    System.out.println(i);

    Person person = new Person("uint256_t");
    person.show();
    Teacher eng = new Teacher("uint256_t", "English");
    eng.show();

    for (i = 1; i <= 50000; i++) {
      if (is_prime(i)) System.out.println(i + " is prime");
    }
  }
}
