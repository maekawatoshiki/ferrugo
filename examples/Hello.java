class Hello {
  public static int add(int max) {
    int sum = 0, i = 1;
    while (i <= max) {
      sum += i;
      i += 1;
    }
    return sum;
  }

  public static void main(String[] args) {
    add(65535);
  }
}
