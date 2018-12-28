class Hello {
  public static int fibo(int n) {
    if (n <= 2) return 1;
    else return fibo(n - 1) + fibo(n - 2);
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
    for (int i = 1; i <= 20; i++)
      System.out.println(fibo(i));
  }
}
