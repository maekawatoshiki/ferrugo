class MillerRabin {
  public static boolean prime_miller(int n) {
    if (n == 2 || n == 3) return true;
    if ((n & 1) == 0)     return false;

    int d = n - 1;
    while ((d & 1) == 0) d = d >> 1;

    for (int k = 0; k < 20; k++) {
      int t = d;
      int q = (int)(Math.random() * (n - 2));
      int x = modpow(1 + q, t, n);

      while (t != n - 1 && x != 1 && x != n - 1) {
        x = modpow(x, 2, n);
        t = t << 1;
      }

      if (x != n - 1 && (t & 1) == 0) return false;
    }

    return true;
  }

  public static int modpow(int base, int power, int mod) {
    int result = 1;
    while (power > 0) {
      if ((power & 1) == 1) result = (result * base) % mod;
      base = (base * base) % mod;
      power = power >> 1;
    }
    return result;
  }

  public static void main(String[] args) {
    for (int i = 2; i < 30000; i++)
      if (prime_miller(i)) System.out.println(i);
  }
}
