class Hello {
  public static int fibo(int n) {
    if (n <= 2) return 1;
    else return fibo(n - 1) + fibo(n - 2);
  }

  public static void main(String[] args) {
    double f = 2.3f;
    f += 1.2f;
    Print.println("Hello World");
    Print.println(123);
    for (int i = 1; i <= 20; i++)
      Print.println(fibo(i));
  }
}
