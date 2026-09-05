class RhinoPlayer < Formula
  desc "Mpv-backed media player with a GTK 4 / libadwaita UI"
  homepage "https://github.com/adrianov/rhino-player"
  # Last tagged stable. Prefer `brew install --HEAD` until v1.7.0 is tagged + sha256 updated.
  url "https://github.com/adrianov/rhino-player/archive/refs/tags/v1.6.1.tar.gz"
  sha256 "7f7da938b2bc3cc0157a5ca5a1d7bdc91f2b6fe451cd3631e793168581b7f3dc"
  license "GPL-3.0-or-later"
  head "https://github.com/adrianov/rhino-player.git", branch: "main"

  depends_on "pkgconf" => :build
  depends_on "rust" => :build
  depends_on "gtk4"
  depends_on "hicolor-icon-theme"
  depends_on "libadwaita"
  depends_on "mpv"

  def install
    # Dev builds use LLD via .cargo/config.toml; Homebrew's clang shim rejects -fuse-ld=lld.
    rm_r ".cargo" if (buildpath/".cargo").exist?
    system "cargo", "install", *std_cargo_args
    man1.install "doc/rhino-player.1"
    install_rhino_share_payload
    install_hicolor_icon_tree
    install_macos_app if OS.mac?
  end

  def install_macos_app
    app = prefix/"Rhino Player.app"
    contents = app/"Contents"
    macos = contents/"MacOS"
    resources = contents/"Resources"
    macos.mkpath
    (resources/"share/rhino-player/vs").mkpath
    (resources/"data/icons").mkpath

    relocate_binary_into_app_bundle(macos)
    install_app_bundle_metadata(contents, resources)
    vendor_mvtools_into_app(resources)
    build_app_icon_icns(resources)
    sign_and_register_macos_app(app)
  end

  post_install_steps do
    update_gtk_icon_cache
  end

  def caveats
    s = <<~EOS
      Smooth Video (~60 FPS) needs MVTools in addition to Homebrew mpv:
        brew install vapoursynth-mvtools
      Then reinstall so the .app can vendor the plugin:
        brew reinstall rhino-player
    EOS
    if OS.mac?
      s += <<~EOS

        Dock / Finder: #{opt_prefix}/Rhino\\ Player.app
        Optional: ln -s "#{opt_prefix}/Rhino Player.app" /Applications/
      EOS
    end
    s
  end

  test do
    assert_match version.to_s, shell_output("#{bin}/rhino-player --version")
    assert_path_exists man1/"rhino-player.1"
    assert_path_exists share/"rhino-player/vs/rhino_60_mvtools.vpy"
    assert_path_exists share/"rhino-player/scripts/macos-vendor-smooth-libs.sh"
    assert_path_exists share/"icons/hicolor/256x256/apps/ch.rhino.RhinoPlayer.png"
    assert_path_exists share/"rhino-player/icons/hicolor/256x256/apps/ch.rhino.RhinoPlayer.png"
    assert_path_exists share/"applications/ch.rhino.RhinoPlayer.desktop"
    return unless OS.mac?

    assert_path_exists prefix/"Rhino Player.app/Contents/Resources/AppIcon.icns"
    assert_path_exists prefix/"Rhino Player.app/Contents/MacOS/rhino-player"
    assert_path_exists prefix/"Rhino Player.app/Contents/Resources/share/rhino-player/vs/rhino_60_mvtools.vpy"
  end

  private

  def install_rhino_share_payload
    (share/"rhino-player/vs").install Dir["data/vs/*.vpy"]
    (share/"rhino-player/scripts").install "scripts/macos-vendor-smooth-libs.sh"
    chmod 0755, share/"rhino-player/scripts/macos-vendor-smooth-libs.sh"
    (share/"applications").install "data/applications/ch.rhino.RhinoPlayer.desktop"
    (share/"metainfo").install "data/metainfo/ch.rhino.RhinoPlayer.metainfo.xml"
  end

  def install_hicolor_icon_tree
    # One Freedesktop tree (app PNG + symbolics). Alias for PREFIX runtime prepend.
    (share/"icons/hicolor").mkpath
    cp_r "data/icons/hicolor/.", share/"icons/hicolor"
    (share/"rhino-player").mkpath
    ln_sf "../icons", share/"rhino-player/icons"
  end

  def relocate_binary_into_app_bundle(macos)
    # Binary inside the bundle so Dock / Launch Services use AppIcon.icns.
    mv bin/"rhino-player", macos/"rhino-player"
    bin.install_symlink macos/"rhino-player"
  end

  def install_app_bundle_metadata(contents, resources)
    inreplace "packaging/macos/Info.plist.in", "@VERSION@", version.to_s
    cp "packaging/macos/Info.plist.in", contents/"Info.plist"
    # share/.../vs was already install'd (moved) from data/vs — copy from the keg share tree.
    cp Dir[share/"rhino-player/vs/*.vpy"], resources/"share/rhino-player/vs/"
    cp_r "data/icons/hicolor", resources/"data/icons/"
  end

  def vendor_mvtools_into_app(resources)
    # Optional Smooth 60: vendor MVTools into the .app when vapoursynth-mvtools is present.
    ENV["SKIP_MISSING"] = "1"
    system "bash", share/"rhino-player/scripts/macos-vendor-smooth-libs.sh",
           (resources/"lib/vapoursynth").to_s
  end

  def build_app_icon_icns(resources)
    iconset = buildpath/"AppIcon.iconset"
    iconset.mkpath
    hicon = buildpath/"data/icons/hicolor"
    macos_app_icon_mappings.each do |name, size|
      cp hicon/"#{size}/apps/ch.rhino.RhinoPlayer.png", iconset/name
    end
    system "iconutil", "-c", "icns", iconset.to_s, "-o", (resources/"AppIcon.icns").to_s
  end

  def macos_app_icon_mappings
    {
      "icon_16x16.png"      => "16x16",
      "icon_16x16@2x.png"   => "32x32",
      "icon_32x32.png"      => "32x32",
      "icon_32x32@2x.png"   => "64x64",
      "icon_128x128.png"    => "128x128",
      "icon_128x128@2x.png" => "256x256",
      "icon_256x256.png"    => "256x256",
      "icon_256x256@2x.png" => "512x512",
      "icon_512x512.png"    => "512x512",
      "icon_512x512@2x.png" => "1024x1024",
    }
  end

  def sign_and_register_macos_app(app)
    system "codesign", "--force", "--sign", "-", "--timestamp=none", app.to_s

    # Prefer this keg over a leftover dist/ or DMG build for the same bundle id.
    lsregister = "/System/Library/Frameworks/CoreServices.framework/Frameworks/" \
                 "LaunchServices.framework/Support/lsregister"
    system lsregister, "-f", app.to_s if File.executable?(lsregister)
  end
end
