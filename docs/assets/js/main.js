const GDPR_STORAGE_KEY = "durin_gdpr_accept_date";
const ONE_YEAR_MS = 365 * 24 * 60 * 60 * 1000;

function safeGetFromStorage(key) {
  try {
    return localStorage.getItem(key);
  } catch (_error) {
    return null;
  }
}

function safeSetToStorage(key, value) {
  try {
    localStorage.setItem(key, value);
  } catch (_error) {
    // Ignore write failures (e.g. strict privacy modes).
  }
}

/**
 * Checks whether GDPR consent modal must be shown.
 * Shows again when acceptance is missing or older than one year.
 */
function shouldShowGdprModal() {
  const saved = safeGetFromStorage(GDPR_STORAGE_KEY);
  if (!saved) {
    return true;
  }

  const acceptedAt = new Date(saved);
  if (Number.isNaN(acceptedAt.getTime())) {
    return true;
  }

  return Date.now() - acceptedAt.getTime() > ONE_YEAR_MS;
}

/**
 * Stores current acceptance date in local storage.
 */
function acceptGdprNotice() {
  safeSetToStorage(GDPR_STORAGE_KEY, new Date().toISOString());
}

/**
 * Initializes and handles GDPR dialog visibility and actions.
 */
function setupGdprModal() {
  const modal = document.getElementById("gdpr-modal");
  const accept = document.getElementById("gdpr-accept");

  if (!modal || !accept) {
    return;
  }

  if (shouldShowGdprModal()) {
    modal.removeAttribute("hidden");
    document.body.style.overflow = "hidden";
  }

  accept.addEventListener("click", () => {
    acceptGdprNotice();
    modal.setAttribute("hidden", "");
    document.body.style.overflow = "";
  });
}

/**
 * Adds reveal-on-scroll animation for all marked sections.
 */
function setupRevealAnimations() {
  const revealNodes = document.querySelectorAll(".reveal");
  if (!revealNodes.length) {
    return;
  }

  const observer = new IntersectionObserver(
    (entries) => {
      for (const entry of entries) {
        if (entry.isIntersecting) {
          entry.target.classList.add("visible");
          observer.unobserve(entry.target);
        }
      }
    },
    {
      threshold: 0.15,
      rootMargin: "0px 0px -30px 0px",
    },
  );

  revealNodes.forEach((node) => observer.observe(node));
}

/**
 * Adds pointer-reactive glow hotspot for feature cards.
 */
function setupCardGlowTracking() {
  const cards = document.querySelectorAll(".glow-card");

  cards.forEach((card) => {
    card.addEventListener("pointermove", (event) => {
      const rect = card.getBoundingClientRect();
      const x = ((event.clientX - rect.left) / rect.width) * 100;
      const y = ((event.clientY - rect.top) / rect.height) * 100;
      card.style.setProperty("--mx", `${x}%`);
      card.style.setProperty("--my", `${y}%`);
    });
  });
}

/**
 * Initializes screenshot lightbox with click, keyboard and button navigation.
 */
function setupScreenshotLightbox() {
  const shots = Array.from(document.querySelectorAll(".shot-grid .shot"));
  const lightbox = document.getElementById("shot-lightbox");
  const lightboxImage = document.getElementById("lightbox-image");
  const lightboxCaption = document.getElementById("lightbox-caption");
  const closeButtons = Array.from(document.querySelectorAll("[data-lightbox-close]"));
  const prevButton = document.querySelector("[data-lightbox-prev]");
  const nextButton = document.querySelector("[data-lightbox-next]");

  if (!shots.length || !lightbox || !lightboxImage || !lightboxCaption) {
    return;
  }

  const slides = shots
    .map((shot) => {
      const image = shot.querySelector("img");
      const captionNode = shot.querySelector("figcaption");
      if (!image) {
        return null;
      }

      return {
        src: image.getAttribute("src") || "",
        alt: image.getAttribute("alt") || "Durin screenshot",
        caption: captionNode ? captionNode.textContent || "" : "",
      };
    })
    .filter(Boolean);

  if (!slides.length) {
    return;
  }

  let currentIndex = 0;

  const showSlide = (index) => {
    const total = slides.length;
    currentIndex = (index + total) % total;
    const current = slides[currentIndex];
    lightboxImage.setAttribute("src", current.src);
    lightboxImage.setAttribute("alt", current.alt);
    lightboxCaption.textContent = current.caption;
  };

  const openLightbox = (index) => {
    showSlide(index);
    lightbox.removeAttribute("hidden");
    document.body.style.overflow = "hidden";
  };

  const closeLightbox = () => {
    lightbox.setAttribute("hidden", "");
    lightboxImage.setAttribute("src", "");
    document.body.style.overflow = "";
  };

  shots.forEach((shot, index) => {
    shot.setAttribute("tabindex", "0");
    shot.setAttribute("role", "button");
    shot.setAttribute("aria-label", "Open screenshot preview");

    shot.addEventListener("click", () => openLightbox(index));
    shot.addEventListener("keydown", (event) => {
      if (event.key === "Enter" || event.key === " ") {
        event.preventDefault();
        openLightbox(index);
      }
    });
  });

  closeButtons.forEach((button) => {
    button.addEventListener("click", closeLightbox);
  });

  if (prevButton) {
    prevButton.addEventListener("click", () => showSlide(currentIndex - 1));
  }

  if (nextButton) {
    nextButton.addEventListener("click", () => showSlide(currentIndex + 1));
  }

  document.addEventListener("keydown", (event) => {
    if (lightbox.hasAttribute("hidden")) {
      return;
    }

    if (event.key === "Escape") {
      closeLightbox();
      return;
    }

    if (event.key === "ArrowLeft") {
      showSlide(currentIndex - 1);
      return;
    }

    if (event.key === "ArrowRight") {
      showSlide(currentIndex + 1);
    }
  });
}

setupGdprModal();
setupRevealAnimations();
setupCardGlowTracking();
setupScreenshotLightbox();
