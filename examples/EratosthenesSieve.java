class EratosthenesSieve {
  public static void main(String args[]) {
    int max = 100000;
    boolean sieve[] = new boolean[max];

    sieve[0] = false;
    for (int i = 1; i < max; i++) sieve[i] = true;

    for (int i = 0; i * i < max; i++) 
      if (sieve[i])
        for (int k = i + 1; (i + 1) * k <= max; k++)
          sieve[(i + 1) * k - 1] = false;

    for (int i = 0; i < max; i++) 
      if (sieve[i]) System.out.println(i + 1);
  }
}
