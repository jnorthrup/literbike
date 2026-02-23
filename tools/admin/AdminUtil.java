package com.talkwheel.server;

import java.security.MessageDigest;
import java.security.NoSuchAlgorithmException;
import java.util.ArrayList;
import java.util.Date;
import java.util.LinkedList;
import java.util.List;
import java.util.Random;
import java.util.concurrent.TimeUnit;

import org.springframework.transaction.annotation.Transactional;

import com.talkwheel.model.TwAvatar;
import com.talkwheel.model.TwItem;
import com.talkwheel.model.TwTopic;

/**
 * A small, sanitized conversation-emulation utility.
 *
 * This class intentionally avoids reproducing any original or copyrighted
 * corpora. It provides a compact, configurable generator of avatars,
 * topics and items useful for tests and local emulation.
 */
public class AdminUtil {
  @SuppressWarnings({"StaticNonFinalField"})
  public static int counter;

  private static final Random RANDOM = new Random(System.currentTimeMillis());

  /** A tiny set of neutral phrase fragments used to assemble messages. */
  public enum Corpus {
    GENERIC(new String[][]{
        {"Hello", "Hi", "Hey", "Greetings", "Good day"},
        {"this is a sample message", "we are testing flow", "please ignore"},
        {"about the topic.", "for demonstration purposes.", "to exercise the system."}
    }),
    BRIEF(new String[][]{
        {"Note:", "FYI:", "Quick:"},
        {"the service is up", "an event occurred", "data was updated"},
        {"check logs.", "no action required.", "monitoring continues."}
    });

    final String[][] strings;

    Corpus(final String[][] strings) {
      // intentionally assign the matrix directly; it is small and safe
      this.strings = strings;
    }
  }

  /**
   * Generate a deterministic MD5 hex for an input string.
   */
  private static String md5Hex(final String input) {
    try {
      final MessageDigest md = MessageDigest.getInstance("MD5");
      byte[] digest = md.digest(input.getBytes());
      StringBuilder sb = new StringBuilder(digest.length * 2);
      for (byte b : digest) {
        sb.append(String.format("%02x", b & 0xff));
      }
      return sb.toString();
    } catch (NoSuchAlgorithmException e) {
      throw new RuntimeException(e);
    }
  }

  /**
   * Create a small conversation session. This method keeps the behaviour and
   * shape similar to legacy utilities but uses neutral, short phrases and
   * generated user names.
   */
  @Transactional
  public void emulateConversation(final Corpus corpus, TwTopic topic,
                                  final int usercount, final int messages, final double days) {
    TwTopic topic1 = topic;
    if (null == topic1) {
      topic1 = new TwTopic();
      topic1.setTitle("session " + new Date());
      topic1.persist();
    }

    TwAvatar[] avatars = new TwAvatar[Math.max(1, usercount)];
    for (int i = 0; i < avatars.length; i++) {
      avatars[i] = new TwAvatar();
      String name = "User" + (i + 1);
      avatars[i].setName(name);
      String email = name.toLowerCase() + "@example.com";
      avatars[i].setMd5(md5Hex(email));
      avatars[i].persist();
    }

    List<TwAvatar> pool = new LinkedList<TwAvatar>();
    double span = TimeUnit.MILLISECONDS.convert((long) (24. * days), TimeUnit.HOURS);
    long startMillis = (long) (System.currentTimeMillis() - span);

    TwItem[] items = new TwItem[Math.max(0, messages)];
    int idx = 0;
    while (idx < items.length) {
      if (pool.isEmpty()) {
        pool.addAll(java.util.Arrays.asList(avatars));
      }

      StringBuilder msg = new StringBuilder();
      // assemble 1..3 phrase lines
      int lines = 1 + RANDOM.nextInt(3);
      for (int l = 0; l < lines; l++) {
        for (int m = 0; m < corpus.strings.length; m++) {
          String[] fragments = corpus.strings[m];
          if (fragments.length == 0) continue;
          if (msg.length() > 0) msg.append(' ');
          msg.append(fragments[RANDOM.nextInt(fragments.length)]);
        }
      }

      items[idx] = new TwItem();
      items[idx].writeHtml(msg.toString());

      TwAvatar creator = pool.remove(RANDOM.nextInt(pool.size()));
      items[idx].setCreator(creator);

      long ts = (long) ((double) startMillis + (double) idx / Math.max(1, items.length) * span);
      items[idx].setTs(new Date(ts));

      // sometimes reply to previous item
      if (idx > 0 && RANDOM.nextDouble() < 0.3) {
        int replyIndex = RANDOM.nextInt(idx);
        items[idx].setReplyto(items[replyIndex]);
        items[idx].setTitle(items[replyIndex].getTitle());
      } else {
        items[idx].setTitle("subject " + counter++);
      }

      items[idx].setTopic(topic1);
      items[idx].persist();
      idx++;
    }
  }

  /**
   * Backwards-compatible wrapper named like the original utility.
   * This keeps the public API shape for callers that expect a
   * "chomskyBot" method while delegating to the sanitized generator.
   */
  @Transactional
  public void chomskyBot(final Corpus corpus, TwTopic topic,
                         final int usercount, final int messages, final double days) {
    // Delegate to the sanitized generator. Behaviour and parameters
    // mimic the original but without reproducing any original text.
    emulateConversation(corpus, topic, usercount, messages, days);
  }
}
