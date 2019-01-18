package java.lang;

public class String {
  private final char value[];
  public static native String valueOf(int n);
  public String() {
    this.value = new char[0];
  }
}
