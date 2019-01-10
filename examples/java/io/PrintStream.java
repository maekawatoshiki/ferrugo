package java.io;

public class PrintStream {
  public PrintStream() { }
  public native void print(String msg);
  public native void println(String msg);
  public native void println(double d);
  public native void println(int i);
}
