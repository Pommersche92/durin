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

setupGdprModal();
setupRevealAnimations();
setupCardGlowTracking();
