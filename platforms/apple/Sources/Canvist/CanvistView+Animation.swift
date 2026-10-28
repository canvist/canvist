import UIKit

/// Animated text: dictation drafts that breathe, final text settling into
/// ink, and kinetic text. The editor animates only while the view asks for a
/// frame on every display refresh, and the view asks only while the last
/// frame said it was animating, so an idle editor costs no frames.
extension CanvistView {
  /// Animate unless the person using the device prefers reduced motion, and
  /// follow the setting when it changes.
  func observeMotionPreference() {
    NotificationCenter.default.addObserver(
      self,
      selector: #selector(motionPreferenceChanged),
      name: UIAccessibility.reduceMotionStatusDidChangeNotification,
      object: nil
    )
    applyMotionPreference(reduced: UIAccessibility.isReduceMotionEnabled)
  }

  @objc private func motionPreferenceChanged() {
    applyMotionPreference(reduced: UIAccessibility.isReduceMotionEnabled)
  }

  /// Turn animations off when motion is `reduced`: drafts then show at a
  /// steady faint opacity, and final text in full ink at once.
  func applyMotionPreference(reduced: Bool) {
    editor.setAnimations(enabled: !reduced)
    setNeedsLayout()
  }

  /// Ask for a frame on every display refresh while `animated`, and stop
  /// as soon as a frame is still.
  func animate(_ animated: Bool) {
    guard animated, window != nil else {
      displayLink?.invalidate()
      displayLink = nil

      return
    }

    guard displayLink == nil else { return }

    let link = CADisplayLink(target: DisplayLinkTarget(view: self), selector: #selector(DisplayLinkTarget.step))
    link.add(to: .main, forMode: .common)
    displayLink = link
  }

  override open func didMoveToWindow() {
    super.didMoveToWindow()

    // Off screen, layout never runs to stop the link: stop it now, and let
    // the next layout pass start it again.
    if window == nil {
      animate(false)
    } else {
      setNeedsLayout()
    }
  }
}

/// The display link's target. A display link holds its target strongly, so
/// the view is held weakly, and the link stops once the view is gone.
private final class DisplayLinkTarget: NSObject {
  private weak var view: CanvistView?

  init(view: CanvistView) {
    self.view = view
  }

  @MainActor @objc func step(_ link: CADisplayLink) {
    guard let view else {
      link.invalidate()

      return
    }

    view.setNeedsLayout()
  }
}
