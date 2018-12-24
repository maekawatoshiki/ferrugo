class Print {
  public static native void println(String s);
  public static void println(int num) {
    println(String.valueOf(num));
  }
}
